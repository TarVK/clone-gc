use std::{
    any::Any,
    cell::{RefCell, RefMut},
    collections::VecDeque,
    rc::Rc,
};

use crate::{
    dirty_list::{DirtyGCPIter, DirtyRoot},
    trace::GCTracer,
    weak_gc_pointer::WeakGCP,
};

#[derive(Clone)]
pub struct GCManager(Rc<RefCell<GCManagerInner>>);

pub struct GCManagerInner {
    pub(crate) dirty_root: DirtyRoot,
    trace_id: u64,
}

/// The main garbage collection functions
impl GCManager {
    pub fn new() -> GCManager {
        GCManager(Rc::new(RefCell::new(GCManagerInner {
            dirty_root: DirtyRoot::new(),
            trace_id: 1, // Start with 1 higher than the default for pointers
        })))
    }
    // Returns the number of items that were garbage collected
    pub fn gc(&self) -> usize {
        let pointers = self.count_cyclic_references();
        self.next_id(); // Indicate the cyclic count trace finished
        let outside_pointers = self.find_outside_pointers(&pointers);
        self.mark_alive_pointers(outside_pointers);
        let dead_values = self.take_dead_values(pointers);
        let mut gc_count = dead_values.len();

        // Drop all values at once to prevent additional GC calls, and only then mark the end of the trace to prevent additional bookkeeping done in the GCP
        drop(dead_values);
        self.next_id();

        // Some cheap memory leak check
        let unexpected_drop = self.inner().dirty_root.next.borrow().is_some();
        if unexpected_drop {
            println!(
                "Breaking dependency chains resulted in unreachable values being dropped! Make sure that Trace can properly reach all children of your data type to prevent memory leaks!"
            );
            gc_count += self.gc();
        }
        gc_count
    }

    pub(crate) fn next_id(&self) -> u64 {
        let mut inner = self.inner();
        inner.trace_id += 1;
        inner.trace_id
    }
    pub(crate) fn trace_id(&self) -> u64 {
        self.inner().trace_id
    }
    fn count_cyclic_references(&self) -> Vec<WeakGCP> {
        let trace_id = self.trace_id();

        let mut pointers = Vec::new();
        let queue = DirtyGCPIter::new(self.inner().dirty_root.take()).collect();
        let mut tracer = GCTracer::new(queue);
        while let Some(pointer) = tracer.queue.pop_back() {
            let already_traced = pointer.with_meta(|meta| {
                // Only increment if this was a pointer (and hence not a root occurrence from being dirty)
                let dirty_data = &mut meta.dirty;
                let increment_ref = (!dirty_data.is_dirty) as usize;
                dirty_data.is_dirty = false;
                dirty_data.prev_dirty = None;
                dirty_data.next_dirty = None;

                // Reset the current to 0 if the count originates from a previous trace
                let already_traced = meta.trace.trace_id == trace_id;
                meta.trace.trace_id = trace_id;
                let cur_count = already_traced as usize * meta.trace.reachable_ref_count;
                meta.trace.reachable_ref_count = cur_count + increment_ref;

                // Assume pointer is not reachable from outside until proven otherwise
                meta.trace.is_reachable = false;
                already_traced
            });
            if already_traced {
                continue;
            }

            // Scan its children
            pointer.trace_content(&mut tracer);
            pointers.push(pointer);
        }

        pointers
    }
    fn find_outside_pointers(&self, pointers: &Vec<WeakGCP>) -> VecDeque<WeakGCP> {
        let mut outside = VecDeque::new();
        for pointer in pointers {
            let ref_count = pointer.get_ref_count();
            pointer.with_meta(|meta| {
                let outside_reachable = meta.trace.reachable_ref_count < ref_count;
                if outside_reachable {
                    outside.push_back(pointer.clone());
                }
            });
        }
        outside
    }
    fn mark_alive_pointers(&self, outside: VecDeque<WeakGCP>) {
        let trace_id = self.trace_id();

        let mut tracer = GCTracer::new(outside);
        while let Some(pointer) = tracer.queue.pop_front() {
            let already_traced = pointer.with_meta(|meta| {
                let already_traced = meta.trace.trace_id == trace_id;
                meta.trace.is_reachable = true;
                meta.trace.trace_id = trace_id;
                already_traced
            });
            if already_traced {
                continue;
            }
            pointer.trace_content(&mut tracer);
        }
    }
    fn take_dead_values(&self, pointers: Vec<WeakGCP>) -> Vec<Box<dyn Any>> {
        let mut dead_values = Vec::new();
        for pointer in pointers {
            let is_dead = pointer.with_meta(|meta| !meta.trace.is_reachable);
            if is_dead {
                dead_values.push(pointer.take_value())
            }
        }
        dead_values
    }

    pub(crate) fn take_dropped_values(&self, value: WeakGCP) -> Vec<Box<dyn Any>> {
        let trace_id = self.trace_id();

        // value.with_meta(|meta| meta.is_dirty = true);
        let mut dead_values = Vec::new();
        let mut tracer = GCTracer::new(VecDeque::from([value]));
        while let Some(pointer) = tracer.queue.pop_back() {
            let ref_count = pointer.get_ref_count();
            let will_drop = pointer.with_meta(|meta| {
                // Reset the current to 0 if the count originates from a previous trace
                let already_traced = meta.trace.trace_id == trace_id;
                meta.trace.trace_id = trace_id;
                let cur_count = already_traced as usize * meta.trace.reachable_ref_count;
                meta.trace.reachable_ref_count = cur_count + 1;

                // Check whether the pointer will die
                let will_drop = meta.trace.reachable_ref_count == ref_count;
                meta.trace.is_reachable = !will_drop;
                will_drop
            });
            if !will_drop {
                continue;
            }

            // Scan its children when dropped, and take the value
            pointer.trace_content(&mut tracer);
            pointer.with_meta(|meta| {
                let dirty_data = &mut meta.dirty;
                if dirty_data.is_dirty {
                    dirty_data.unmark_dirty();
                }
            });
            dead_values.push(pointer.take_value());
        }

        self.next_id();
        dead_values
    }
}

impl GCManager {
    pub(crate) fn inner(&self) -> RefMut<'_, GCManagerInner> {
        self.0.borrow_mut()
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
