use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crate::{
    gc_pointer::{DirtyGCPIter, DynGCP, WeakGCP},
    trace::GCTracer,
};

#[derive(Clone)]
pub struct GCManager(Rc<RefCell<GCManagerInner>>);

pub struct GCManagerInner {
    dirty_root: Option<WeakGCP>,
    trace_id: u64,
}

/// The main garbage collection functions
impl GCManager {
    pub fn new() -> GCManager {
        GCManager(Rc::new(RefCell::new(GCManagerInner {
            dirty_root: None,
            trace_id: 0,
        })))
    }

    pub fn gc(&self) {
        let mut inner = self.inner();
        let pointers = inner.count_cyclic_references();
        let dead_pointers = inner.find_unreachable_pointers(pointers);
        drop(inner);
        // Perform value dropping after dropping inner, to allow value dropping side-effects to use gc-inner.
        for pointer in dead_pointers {
            pointer.drop_value();
        }
    }
}

impl GCManager {
    pub(crate) fn inner(&self) -> RefMut<'_, GCManagerInner> {
        self.0.borrow_mut()
    }
}

impl GCManagerInner {
    pub(crate) fn mark_dirty(&mut self, p: WeakGCP) {
        let mut meta = p.meta();
        if let Some(next_dirty) = self.dirty_root.take() {
            let mut meta_next = next_dirty.meta();
            (*meta_next).prev_dirty = Some(p.clone());
            drop(meta_next);
            meta.next_dirty = Some(next_dirty);
        }
        meta.is_dirty = true;
        drop(meta);
        self.dirty_root = Some(p);
    }
    pub(crate) fn unmark_dirty(&mut self, p: WeakGCP) {
        let mut meta = p.meta();

        let maybe_prev = meta.prev_dirty.take();
        let maybe_next = meta.next_dirty.take();
        if let Some(prev) = &maybe_prev {
            let mut prev_meta = prev.meta();
            prev_meta.next_dirty = maybe_next.clone();
        } else {
            // If there's no previous, this was the first element
            self.dirty_root = maybe_next.clone();
        }
        if let Some(next) = maybe_next {
            let mut next_meta = next.meta();
            next_meta.prev_dirty = maybe_prev;
        }

        meta.is_dirty = false;
    }
    fn count_cyclic_references(&mut self) -> Vec<WeakGCP> {
        self.trace_id += 1;
        let trace_id = self.trace_id;

        // Initialize the roots to have no reached incoming references
        let mut queue = Vec::new();
        for pointer in DirtyGCPIter::new(self.dirty_root.take()) {
            let mut meta = pointer.meta();
            meta.trace.id = trace_id;
            meta.trace.ref_count = 0;
            meta.prev_dirty = None;
            meta.next_dirty = None;
            meta.is_dirty = false;
            drop(meta);
            queue.push(pointer);
        }

        // Iterate over all reachable nodes
        let mut pointers = Vec::new();
        let mut tracer = GCTracer::new(trace_id, true, queue);
        while let Some(pointer) = tracer.queue.pop() {
            pointer.trace_content(&mut tracer);

            // Register pointers as dead, until proven otherwise
            pointer.meta().trace.dead = true;
            pointers.push(pointer);
        }

        pointers
    }
    fn find_unreachable_pointers(&mut self, pointers: Vec<WeakGCP>) -> Vec<WeakGCP> {
        self.trace_id += 1;
        let trace_id = self.trace_id;

        // Find all pointers directly accessed from outside this set
        let mut reachable = Vec::new();
        for pointer in &pointers {
            let meta = pointer.meta();
            let is_reachable = meta.trace.ref_count < meta.ref_count;
            drop(meta);
            if is_reachable {
                reachable.push(pointer.clone());
            }
        }

        // Mark all externally reachable pointers as alive
        let mut tracer = GCTracer::new(trace_id, false, reachable);
        while let Some(pointer) = tracer.queue.pop() {
            pointer.meta().trace.dead = false;
            pointer.trace_content(&mut tracer);
        }

        // Find all dead pointers
        let mut dead = Vec::new();
        for pointer in pointers {
            if !pointer.meta().trace.dead {
                continue;
            }

            dead.push(pointer);
        }
        dead
    }
}

pub trait GetGCManager {
    fn get_manager(&self) -> GCManager;
}
impl GetGCManager for GCManager {
    fn get_manager(&self) -> GCManager {
        self.clone()
    }
}
