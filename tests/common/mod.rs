use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    ops::Deref,
    rc::Rc,
};

use clone_gc::{Field, GCP, GCTracer, GetGCManager, Trace};

#[derive(Clone)]
pub struct BiGraph(GCP<BiGraphInner>);
pub struct BiGraphInner {
    pub first: Field<Option<BiGraph>>,
    pub second: Field<Option<BiGraph>>,
    pub id: usize,
    tracker: Tracker,
}
pub type Tracker = Rc<RefCell<Vec<usize>>>;
impl BiGraph {
    pub fn new<T: GetGCManager>(manager: &T, tracker: Tracker, id: usize) -> BiGraph {
        BiGraph(GCP::new(
            manager,
            BiGraphInner {
                first: Field::new(None),
                second: Field::new(None),
                id,
                tracker,
            },
        ))
    }
}
impl Deref for BiGraph {
    type Target = BiGraphInner;
    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
impl Trace for BiGraph {
    fn trace(&self, tracer: &mut GCTracer) {
        self.0.trace(tracer);
    }
}
impl Trace for BiGraphInner {
    fn trace(&self, tracer: &mut GCTracer) {
        self.first.trace(tracer);
        self.second.trace(tracer);
    }
}

impl Display for BiGraph {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Display for BiGraphInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match (&*self.first.get(), &*self.second.get()) {
            (None, None) => write!(f, "{}", self.id),
            (None, Some(s)) => write!(f, "{} -> {}", self.id, s.id),
            (Some(n), None) => write!(f, "{} -> {}", self.id, n.id),
            (Some(n), Some(s)) => {
                write!(f, "{} -> {};{}", self.id, n.id, s.id)
            }
        }
    }
}
impl Drop for BiGraphInner {
    fn drop(&mut self) {
        self.tracker.borrow_mut().push(self.id);
    }
}
