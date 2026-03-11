#[allow(dead_code)]
enum LL {
    Next(Box<LL>),
    End,
}

// #[test]
pub fn ll_drop_crashes() {
    const SIZE: usize = 100000;

    let ll = (0..SIZE).fold(LL::End, |n, _i| LL::Next(Box::new(n)));

    drop(ll);
}

use std::{cell::RefCell, rc::Rc};

use clone_gc::{Field, GCManager, GCP, Trace};

type GCLL = GCP<GCLLInner>;
type DropCount = Rc<RefCell<usize>>;
enum GCLLInner {
    Next(GCLL, DropCount),
    End(Field<Option<GCLL>>),
}
impl Trace for GCLLInner {
    fn trace(&self, tracer: &mut clone_gc::GCTracer) {
        match self {
            GCLLInner::Next(gcp, _) => tracer.mark(gcp),
            GCLLInner::End(m) => m.trace(tracer),
        }
    }
}
impl Drop for GCLLInner {
    fn drop(&mut self) {
        match self {
            GCLLInner::Next(_, ref_cell) => *ref_cell.borrow_mut() += 1,
            GCLLInner::End(_) => (),
        }
    }
}

#[test]
pub fn gc_ll_drop_cycle() {
    gc_drop_maybe_cycle(true);
}
#[test]
pub fn gc_ll_drop_no_cycle() {
    gc_drop_maybe_cycle(false);
}

pub fn gc_drop_maybe_cycle(cycle: bool) {
    const SIZE: usize = 1000_000;
    let drop_count = Rc::new(RefCell::new(0));
    let gc = GCManager::new();
    let end = GCP::new(&gc, GCLLInner::End(Field::new(None)));
    let ll = (0..SIZE).fold(end.clone(), |n, _i| {
        GCP::new(&gc, GCLLInner::Next(n, drop_count.clone()))
    });
    if cycle {
        if let GCLLInner::End(field) = &*end {
            field.set(Some(ll.clone()));
        }
    }

    drop(ll);
    drop(end);
    if cycle {
        gc.gc();
    }
    assert_eq!(*drop_count.borrow(), SIZE);
}
