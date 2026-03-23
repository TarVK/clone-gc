use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
};

use crate::{DRc, gc_pointer::GCP, weak_gc_pointer::WeakGCP};

pub trait Trace {
    fn trace(&self, tracer: &mut GCTracer);
}

pub struct GCTracer {
    pub(crate) queue: VecDeque<WeakGCP>,
}
impl GCTracer {
    pub(crate) fn new(queue: VecDeque<WeakGCP>) -> Self {
        GCTracer { queue }
    }
    #[inline]
    pub fn mark<V: Trace>(&mut self, pointer: &GCP<V>) {
        self.queue.push_back(pointer.clone_weak());
    }
}

impl<V: Trace> Trace for RefCell<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        self.borrow().trace(tracer);
    }
}
impl<V: Trace> Trace for Option<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        match self {
            Some(val) => val.trace(tracer),
            None => (),
        }
    }
}
impl<V: Trace> Trace for Box<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        self.as_ref().trace(tracer);
    }
}
impl<V: Trace> Trace for GCP<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        tracer.mark(&self);
    }
}
impl<X: Trace> Trace for Vec<X> {
    fn trace(&self, tracer: &mut GCTracer) {
        self.iter().for_each(|v| v.trace(tracer));
    }
}
impl<X: Trace> Trace for HashSet<X> {
    fn trace(&self, tracer: &mut GCTracer) {
        self.iter().for_each(|v| v.trace(tracer));
    }
}
impl<X: Trace, Y: Trace> Trace for HashMap<X, Y> {
    fn trace(&self, tracer: &mut GCTracer) {
        self.iter().for_each(|(k, v)| {
            k.trace(tracer);
            v.trace(tracer)
        });
    }
}

// Dummy traces to reduce manual skipping of fields
impl<V> Trace for DRc<V> {
    fn trace(&self, _tracer: &mut GCTracer) {
        // No tracing needed
    }
}
impl<V> Trace for Rc<V> {
    fn trace(&self, _tracer: &mut GCTracer) {
        // No tracing needed
    }
}
macro_rules! impl_trace {
    ($type:tt) => {
        impl Trace for $type {
            fn trace(&self, _tracer: &mut GCTracer) {
                // No tracing needed
            }
        }
    };
}
impl_trace!(bool);
impl_trace!(f32);
impl_trace!(f64);
impl_trace!(u8);
impl_trace!(u16);
impl_trace!(u32);
impl_trace!(u64);
impl_trace!(usize);
impl_trace!(i8);
impl_trace!(i16);
impl_trace!(i32);
impl_trace!(i64);
impl_trace!(isize);
