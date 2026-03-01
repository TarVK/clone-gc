use std::{collections::VecDeque, rc::Rc};

use crate::{gc_pointer::GCP, weak_gc_pointer::WeakGCP};

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

impl<V: Trace> Trace for Rc<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        let inner = &**self;
        inner.trace(tracer)
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
