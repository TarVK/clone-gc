use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    ops::Deref,
    rc::Rc,
};

use clone_gc::{Field, GCP, GCTracer, GetGCManager, GraphClone, Trace};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BiGraph(GCP<BiGraphInner>);
pub struct BiGraphInner {
    pub first: Field<Option<BiGraph>>,
    pub second: Field<Option<BiGraph>>,
    pub id: Field<usize>,
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
                id: Field::new(id),
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
            (None, None) => write!(f, "{}", self.id.get()),
            (None, Some(s)) => write!(f, "{} -> {}", self.id.get(), s.id.get()),
            (Some(n), None) => write!(f, "{} -> {}", self.id.get(), n.id.get()),
            (Some(n), Some(s)) => {
                write!(f, "{} -> {};{}", self.id.get(), n.id.get(), s.id.get())
            }
        }
    }
}
impl Drop for BiGraphInner {
    fn drop(&mut self) {
        self.tracker.borrow_mut().push(*self.id.get());
    }
}

impl Into<GCP<BiGraphInner>> for BiGraph {
    fn into(self) -> GCP<BiGraphInner> {
        self.0
    }
}
impl From<GCP<BiGraphInner>> for BiGraph {
    fn from(value: GCP<BiGraphInner>) -> Self {
        BiGraph(value)
    }
}

impl GraphClone for BiGraph {
    fn graph_clone(&self, m: &mut clone_gc::GraphCloneState) -> Self {
        Self(self.0.graph_clone(m))
    }
}
impl GraphClone for BiGraphInner {
    fn graph_clone(&self, m: &mut clone_gc::GraphCloneState) -> Self {
        Self {
            first: self.first.graph_clone(m),
            second: self.second.graph_clone(m),
            id: self.id.graph_clone(m),
            tracker: self.tracker.clone(),
        }
    }
}
