use std::{
    cell::{RefCell, RefMut},
    ops::Deref,
    rc::Rc,
};

use crate::{
    dirty_list::DirtyData,
    gc_manager::{GCManager, GetGCManager},
    trace::Trace,
};

pub struct GCP<V: Trace + 'static>(pub(crate) Rc<GCPInner<V>>); // A Garbage Collected Pointer should be cloneable cheaply to create a new pointer

pub struct GCPInner<V: Trace> {
    pub(crate) gc_meta: RefCell<GCData>,
    pub(crate) value: Option<V>, // A GC pointer should allow its value to be nulled to break cycles
}

/// Core GCP usage functions
impl<V: Trace + 'static> GCP<V> {
    pub fn new<M: GetGCManager>(manager_ref: &M, val: V) -> GCP<V> {
        GCP(Rc::new(GCPInner {
            value: Some(val),
            gc_meta: RefCell::new(GCData {
                trace: TraceData {
                    trace_id: 0,
                    reachable_ref_count: 0,
                    is_reachable: true,
                },
                gc: manager_ref.get_manager(),
                dirty: DirtyData {
                    is_dirty: false,
                    prev_dirty: None,
                    next_dirty: None,
                },
            }),
        }))
    }

    /// Creates a new garbage collected point, belonging to the same garbage collector as this value
    pub fn ptr<T: Trace>(&self, val: T) -> GCP<T> {
        GCP::<T>::new(self, val)
    }
    pub(crate) fn meta(&self) -> RefMut<'_, GCData> {
        self.0.gc_meta.borrow_mut()
    }
}
impl<V: Trace + 'static> Deref for GCP<V> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        self.0.value.as_ref().expect("Cannot access a reference after it has been GCed (do not access child GCPs when a value drops)")
    }
}
impl<V: Trace> PartialEq for GCP<V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<V: Trace> Eq for GCP<V> {}
impl<V: Trace> Clone for GCP<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<V: Trace + 'static> GetGCManager for GCP<V> {
    fn get_manager(&self) -> GCManager {
        self.meta().gc.clone()
    }
}

/// When dropping a GCP, we should mark it for garbage collection
impl<V: Trace + 'static> Drop for GCP<V> {
    fn drop(&mut self) {
        let mut meta = self.meta();
        let sweeping = meta.trace.trace_id == meta.gc.trace_id();
        if sweeping || !meta.trace.is_reachable {
            // If we are in a sweep, we have already determined whether or not this value will remain reachable, hence dirtying is not needed
            return;
        }

        let ref_count = Rc::strong_count(&self.0);
        let will_drop = ref_count <= 1;
        if !will_drop && !meta.dirty.is_dirty {
            let gc = meta.gc.clone();
            meta.dirty
                .mark_dirty(&mut gc.inner().dirty_root, self.clone_weak());
        }

        #[cfg(not(feature = "iterative-drop"))]
        if will_drop && meta.is_dirty {
            meta.unmark_dirty();
        }

        #[cfg(feature = "iterative-drop")]
        // If the data drops, perform a non recursive drop
        if will_drop {
            let gc = meta.gc.clone();
            drop(meta);
            let values = gc.take_dropped_values(self.clone_weak());
            drop(values);
        }
    }
}

pub(crate) struct GCData {
    pub trace: TraceData,
    pub gc: GCManager,
    pub dirty: DirtyData,
}
pub(crate) struct TraceData {
    pub trace_id: u64,
    pub reachable_ref_count: usize,
    pub is_reachable: bool,
}
