use std::{
    any::Any,
    cell::RefMut,
    rc::{Rc, Weak},
};

use crate::{
    GCP, GCTracer, Trace,
    gc_pointer::{GCData, GCPInner},
};

#[derive(Clone)]
pub(crate) struct WeakGCP(pub(crate) Weak<dyn DynGCP>);
pub(crate) trait DynGCP {
    fn meta(&self) -> RefMut<'_, GCData>;
    fn take_value(&self) -> Box<dyn Any>;
    fn trace_content(&self, tracer: &mut GCTracer);
}
impl<V: Trace + 'static> DynGCP for GCPInner<V> {
    fn meta(&self) -> RefMut<'_, GCData> {
        self.gc_meta.borrow_mut()
    }
    fn take_value(&self) -> Box<dyn Any> {
        unsafe {
            let ptr = self as *const GCPInner<V> as *mut GCPInner<V>;
            Box::new((*ptr).value.take())
        }
    }
    fn trace_content(&self, tracer: &mut GCTracer) {
        self.value
            .as_ref()
            .expect("Cannot trace after destructing")
            .trace(tracer);
    }
}
impl WeakGCP {
    pub fn with_meta<R, F: FnOnce(&mut GCData) -> R>(&self, func: F) -> R {
        let strong = self.0.upgrade().unwrap();
        func(&mut *strong.meta())
    }
    pub fn take_value(&self) -> Box<dyn Any> {
        self.0
            .upgrade()
            .map(|strong| strong.take_value())
            .unwrap_or(Box::new(()))
    }
    pub fn trace_content(&self, tracer: &mut GCTracer) {
        let strong = self.0.upgrade().unwrap();
        strong.trace_content(tracer);
    }
    pub fn get_ref_count(&self) -> usize {
        Weak::strong_count(&self.0)
    }
}

impl<V: Trace + 'static> GCP<V> {
    /// Clones the GCP, without reference counting. Only used for internal GC usage
    pub(crate) fn clone_weak(&self) -> WeakGCP {
        let rc: Rc<dyn DynGCP> = self.0.clone();
        WeakGCP(Rc::downgrade(&rc))
    }
}
