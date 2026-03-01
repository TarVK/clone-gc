use std::{cell::RefCell, rc::Rc};

use crate::{GCTracer, Trace};

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
        self.get().trace(tracer);
    }
}
