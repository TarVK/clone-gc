use std::{cell::RefCell, rc::Rc};

use crate::weak_gc_pointer::WeakGCP;

// Doubly linked list to track dirty objects
pub(crate) struct DirtyData {
    pub is_dirty: bool,
    pub prev_dirty: Option<PrevDirty>,
    pub next_dirty: Option<WeakGCP>,
}
impl DirtyData {
    pub(crate) fn unmark_dirty(&mut self) {
        assert!(self.is_dirty);
        self.is_dirty = false;
        let maybe_previous = self.prev_dirty.take();
        let maybe_next = self.next_dirty.take();

        if let Some(previous) = &maybe_previous {
            previous.set_next(maybe_next.clone());
        }
        if let Some(next) = &maybe_next {
            next.with_meta(|next_meta| {
                next_meta.dirty.prev_dirty = maybe_previous;
            });
        }
    }
    pub(crate) fn mark_dirty(&mut self, root: &mut DirtyRoot, gcp: WeakGCP) {
        assert!(!self.is_dirty);
        self.is_dirty = true;
        let maybe_next = root.next.replace(Some(gcp.clone()));
        if let Some(next) = &maybe_next {
            next.with_meta(|next_meta| next_meta.dirty.prev_dirty = Some(PrevDirty::Pointer(gcp)))
        }
        self.prev_dirty = Some(PrevDirty::Root(root.clone()));
        self.next_dirty = maybe_next;
    }
}

// The dirty root
#[derive(Clone)]
pub(crate) struct DirtyRoot {
    pub next: Rc<RefCell<Option<WeakGCP>>>,
}
impl DirtyRoot {
    pub fn new() -> Self {
        DirtyRoot {
            next: Rc::new(RefCell::new(None)),
        }
    }
    pub fn take(&self) -> Option<WeakGCP> {
        self.next.borrow_mut().take()
    }
}

// The previous dirty item, either the root or a pointer
pub(crate) enum PrevDirty {
    Root(DirtyRoot),
    Pointer(WeakGCP),
}
impl PrevDirty {
    fn set_next(&self, next: Option<WeakGCP>) {
        match self {
            PrevDirty::Root(dirty_root) => {
                *dirty_root.next.borrow_mut() = next;
            }
            PrevDirty::Pointer(weak_gcp) => {
                weak_gcp.with_meta(|prev| prev.dirty.next_dirty = next);
            }
        }
    }
}

/// Dirty iterator
pub(crate) struct DirtyGCPIter(Option<WeakGCP>);
impl DirtyGCPIter {
    pub fn new(root: Option<WeakGCP>) -> DirtyGCPIter {
        DirtyGCPIter(root)
    }
}
impl Iterator for DirtyGCPIter {
    type Item = WeakGCP;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.take().map(|next| {
            next.with_meta(|next_meta| self.0 = next_meta.dirty.next_dirty.clone());
            next
        })
    }
}
