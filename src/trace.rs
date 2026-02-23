use crate::gc_pointer::{GCP, WeakGCP};

pub trait Trace {
    fn trace(&self, tracer: &mut GCTracer);
}

pub struct GCTracer {
    pub(crate) queue: Vec<WeakGCP>,
    pub(crate) perform_count: bool,
    pub(crate) trace_id: u64,
}
impl GCTracer {
    pub(crate) fn new(trace_id: u64, perform_count: bool, queue: Vec<WeakGCP>) -> Self {
        GCTracer {
            queue,
            perform_count,
            trace_id,
        }
    }
    #[inline]
    pub fn mark<V: Trace>(&mut self, pointer: &GCP<V>) {
        let mut meta = pointer.meta();
        let already_reached = meta.trace.id == self.trace_id;
        if !already_reached {
            meta.trace.id = self.trace_id;
            self.queue.push(pointer.clone_weak());
            if self.perform_count {
                meta.trace.ref_count = 1;
            }
        } else if self.perform_count {
            meta.trace.ref_count += 1;
        }
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
