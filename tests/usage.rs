use std::{
    cell::RefCell,
    fmt::{Display, Formatter},
    rc::Rc,
};

use clone_gc::{GCManager, GCP, GCTracer, GetGCManager, Trace};

#[test]
fn usage1() {
    type BiGraph = GCP<BiGraphInner>;
    struct BiGraphInner {
        first: GCP<Option<BiGraph>>,
        second: GCP<Option<BiGraph>>,
        name: String,
        tracker: Tracker,
    }
    type Tracker = Rc<RefCell<Vec<String>>>;
    impl BiGraphInner {
        pub fn new<T: GetGCManager>(manager: &T, tracker: Tracker, name: &str) -> BiGraph {
            GCP::new(
                manager,
                BiGraphInner {
                    first: GCP::new(manager, None),
                    second: GCP::new(manager, None),
                    name: name.into(),
                    tracker,
                },
            )
        }
    }
    impl Display for BiGraphInner {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match (&*self.first.get(), &*self.second.get()) {
                (None, None) => write!(f, "{}", self.name),
                (None, Some(s)) => write!(f, "{} -> {}", self.name, s.get().name),
                (Some(n), None) => write!(f, "{} -> {}", self.name, n.get().name),
                (Some(n), Some(s)) => {
                    write!(f, "{} -> {};{}", self.name, n.get().name, s.get().name)
                }
            }
        }
    }
    impl Drop for BiGraphInner {
        fn drop(&mut self) {
            self.tracker.borrow_mut().push(self.name.clone());
        }
    }
    impl Trace for BiGraphInner {
        fn trace(&self, tracer: &mut GCTracer) {
            tracer.mark(&self.first);
            tracer.mark(&self.second);
        }
    }

    let gc = GCManager::new();

    let destroyed = Rc::new(RefCell::new(Vec::new()));
    let v1 = BiGraphInner::new(&gc, destroyed.clone(), "v1");
    let v2 = BiGraphInner::new(&gc, destroyed.clone(), "v2");
    let v3 = BiGraphInner::new(&gc, destroyed.clone(), "v3");
    let v4 = BiGraphInner::new(&gc, destroyed.clone(), "v4");
    let v5 = BiGraphInner::new(&gc, destroyed.clone(), "v5");
    v1.get().second.set(Some(v2.clone()));
    v2.get().second.set(Some(v3.clone()));
    v3.get().second.set(Some(v4.clone()));
    v4.get().second.set(Some(v2.clone()));
    v4.get().first.set(Some(v5.clone()));

    (*v4.get().second.get())
        .as_ref()
        .unwrap()
        .get()
        .first
        .set(Some(v1.clone()));

    println!(
        "{}",
        [v1.clone(), v2.clone(), v3.clone(), v4.clone(), v5.clone()]
            .iter()
            .map(|s| format!("{}", s.get()))
            .collect::<Vec<_>>()
            .join(",\n")
    );

    let destroyed_text = || destroyed.borrow().join(",");
    drop(v5);
    assert_eq!(destroyed_text(), "");
    v4.get().first.set(None);
    assert_eq!(destroyed_text(), "v5");
    drop((v2, v3, v4));
    assert_eq!(destroyed_text(), "v5");
    gc.gc();
    assert_eq!(destroyed_text(), "v5");
    // Replace v2 -> v3 by None
    {
        (*v1.get().second.get())
            .as_ref()
            .unwrap()
            .get()
            .second
            .set(None);
    }
    assert_eq!(destroyed_text(), "v5,v3,v4");
    drop(v1);
    assert_eq!(destroyed_text(), "v5,v3,v4");
    gc.gc();
    assert_eq!(destroyed_text(), "v5,v3,v4,v2,v1");
}
