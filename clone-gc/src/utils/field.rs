use std::{cell::RefCell, hash::Hash, rc::Rc};

use serde::{Deserialize, Serialize};

use crate::{GCTracer, GraphClone, GraphCloneState, Trace};

#[derive(Default, Eq, PartialEq, PartialOrd, Ord, Clone)]
pub struct Field<X>(RefCell<Rc<X>>);

impl<X> Field<X> {
    pub fn new(val: X) -> Self {
        Field(RefCell::new(Rc::new(val)))
    }
    pub fn from<V: Into<X>>(val: V) -> Self {
        Self::new(val.into())
    }
    pub fn get(&self) -> Rc<X> {
        self.0.borrow().clone()
    }
    pub fn set(&self, value: X) {
        *self.0.borrow_mut() = Rc::new(value);
    }
}

impl<V: Trace> Trace for Field<V> {
    fn trace(&self, tracer: &mut GCTracer) {
        (*self.get()).trace(tracer);
    }
}
impl<V: GraphClone> GraphClone for Field<V> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        Self::new((*self.get()).graph_clone(m))
    }
}

impl<X> Hash for Field<X>
where
    X: Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl<X> Serialize for Field<X>
where
    X: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.get().serialize(serializer)
    }
}

impl<'de, X> Deserialize<'de> for Field<X>
where
    X: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::new(X::deserialize(deserializer)?))
    }
}
