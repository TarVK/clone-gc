use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crate::{
    gc_manager::{GCManager, GetGCManager},
    trace::{GCTracer, Trace},
};

pub struct GCP<V: Trace + 'static>(pub(crate) Rc<GCPInner<V>>); // A Garbage Collected Pointer should be cloneable cheaply to create a new pointer

pub struct GCPInner<V: Trace> {
    pub(crate) value: RefCell<Option<GCPVal<V>>>, // A GC pointer should allow its value to be updated, or even deleted when garbage collected to break cycles
    pub(crate) gc_meta: RefCell<GCData>,
}
type GCPVal<V> = Rc<V>; // A GC pointer should ensure that access to old references continue working

/// Core GCP usage functions
impl<V: Trace + 'static> GCP<V> {
    pub fn new<M: GetGCManager>(manager_ref: &M, val: V) -> GCP<V> {
        GCP(Rc::new(GCPInner {
            value: RefCell::new(Some(Rc::new(val))),
            gc_meta: RefCell::new(GCData {
                ref_count: 1,
                trace: TraceData {
                    id: 0,
                    ref_count: 0,
                    dead: false,
                },
                gc: manager_ref.get_manager(),
                is_dirty: false,
                prev_dirty: None,
                next_dirty: None,
            }),
        }))
    }

    /// Creates a new garbage collected point, belonging to the same garbage collector as this value
    pub fn ptr<T: Trace>(&self, val: T) -> GCP<T> {
        GCP::<T>::new(self, val)
    }

    /// Updates the value that this pointer points to
    pub fn set(&self, val: V) {
        *self.0.value.borrow_mut() = Some(Rc::new(val));
    }

    /// Accesses the value this pointer points to
    pub fn get(&self) -> Rc<V> {
        if self.meta().trace.dead {
            // During a drop, a value may get access to a pointer already marked for deletion.
            panic!("GCed objects may not be accessed during drop");
        }
        (*self.0.value.borrow()).as_ref().unwrap().clone()
    }
}
// impl<V: Trace + 'static> Deref for GCP<V> {
//     type Target = V;

//     fn deref(&self) -> &Self::Target {

//     }
// }

impl<V: Trace + 'static> GetGCManager for GCP<V> {
    fn get_manager(&self) -> GCManager {
        self.meta().gc.clone()
    }
}
impl<V: Trace> PartialEq for GCP<V> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<V: Trace> Eq for GCP<V> {}

/// When dropping a GCP, we should mark it for garbage collection
impl<V: Trace + 'static> Drop for GCP<V> {
    fn drop(&mut self) {
        let mut meta = self.meta();
        if meta.trace.dead {
            return;
        }
        let new_count = meta.ref_count - 1;
        meta.ref_count = new_count;
        if new_count == 0 {
            if meta.is_dirty {
                let gc = meta.gc();
                drop(meta);
                gc.inner().unmark_dirty(self.clone_weak());
            }
        } else {
            if !meta.is_dirty {
                let gc = meta.gc();
                drop(meta);
                gc.inner().mark_dirty(self.clone_weak());
            }
        }
    }
}
impl<V: Trace> Clone for GCP<V> {
    fn clone(&self) -> Self {
        let mut meta = self.meta();
        meta.ref_count += 1;
        drop(meta);
        GCP(self.0.clone())
    }
}

pub(crate) struct GCData {
    pub ref_count: usize,
    pub trace: TraceData,
    pub gc: GCManager,

    // Doubly linked list to track dirty objects
    pub is_dirty: bool,
    pub prev_dirty: Option<WeakGCP>,
    pub next_dirty: Option<WeakGCP>,
}

pub(crate) struct TraceData {
    pub id: u64,
    pub ref_count: usize,
    pub dead: bool,
}
impl GCData {
    pub fn gc(&self) -> GCManager {
        self.gc.clone()
    }
}

impl<V: Trace> GCP<V> {
    pub(crate) fn meta(&self) -> RefMut<'_, GCData> {
        self.0.gc_meta.borrow_mut()
    }
}
impl<V: Trace + 'static> GCP<V> {
    /// Clones the GCP, without reference counting. Only used for internal GC usage
    pub(crate) fn clone_weak(&self) -> WeakGCP {
        WeakGCP(self.0.clone())
    }
}

pub struct WeakGCP(pub(crate) Rc<dyn DynGCP>);
pub(crate) trait DynGCP {
    fn meta(&self) -> RefMut<'_, GCData>;
    fn drop_value(&self);
    fn trace_content(&self, tracer: &mut GCTracer);
}
impl<V: Trace> DynGCP for GCPInner<V> {
    fn meta(&self) -> RefMut<'_, GCData> {
        self.gc_meta.borrow_mut()
    }

    fn drop_value(&self) {
        *self.value.borrow_mut() = None;
    }
    fn trace_content(&self, tracer: &mut GCTracer) {
        if let Some(val) = &self.value.borrow().as_ref() {
            val.trace(tracer);
        }
    }
}

impl DynGCP for WeakGCP {
    fn meta(&self) -> RefMut<'_, GCData> {
        self.0.meta()
    }

    fn drop_value(&self) {
        self.0.drop_value();
    }
    fn trace_content(&self, tracer: &mut GCTracer) {
        self.0.trace_content(tracer);
    }
}

impl PartialEq for WeakGCP {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for WeakGCP {}
impl Clone for WeakGCP {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
pub(crate) struct DirtyGCPIter(Option<WeakGCP>);
impl DirtyGCPIter {
    pub fn new(root: Option<WeakGCP>) -> DirtyGCPIter {
        DirtyGCPIter(root)
    }
}
impl Iterator for DirtyGCPIter {
    type Item = WeakGCP;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.0.take() {
            let meta = next.meta();
            self.0 = meta.next_dirty.clone();
            drop(meta);
            Some(next)
        } else {
            None
        }
    }
}
