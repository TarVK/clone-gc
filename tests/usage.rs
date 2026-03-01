use std::{cell::RefCell, rc::Rc};

use clone_gc::GCManager;
mod common;
use common::*;

#[test]
fn usage1() {
    let gc = GCManager::new();

    let destroyed = Rc::new(RefCell::new(Vec::new()));
    let v1 = BiGraph::new(&gc, destroyed.clone(), 1);
    let v2 = BiGraph::new(&gc, destroyed.clone(), 2);
    let v3 = BiGraph::new(&gc, destroyed.clone(), 3);
    let v4 = BiGraph::new(&gc, destroyed.clone(), 4);
    let v5 = BiGraph::new(&gc, destroyed.clone(), 5);
    v1.second.set(Some(v2.clone()));
    v2.second.set(Some(v3.clone()));
    v3.second.set(Some(v4.clone()));
    v4.second.set(Some(v2.clone()));
    v4.first.set(Some(v5.clone()));

    (*v4.second.get())
        .as_ref()
        .unwrap()
        .first
        .set(Some(v1.clone()));

    println!(
        "{}",
        [&v1, &v2, &v3, &v4, &v5]
            .iter()
            .map(|s| format!("{}", s))
            .collect::<Vec<_>>()
            .join(",\n")
    );

    let destroyed_text = || {
        destroyed
            .borrow()
            .iter()
            .map(|v| format!("{}", v))
            .collect::<Vec<_>>()
            .join(",")
    };
    drop(v5);
    assert_eq!(destroyed_text(), "");
    v4.first.set(None);
    assert_eq!(destroyed_text(), "5");
    drop((v2, v3, v4));
    assert_eq!(destroyed_text(), "5");
    gc.gc();
    assert_eq!(destroyed_text(), "5");
    // Replace v2 -> v3 by None
    {
        (*v1.second.get()).as_ref().unwrap().second.set(None);
    }
    assert_eq!(destroyed_text(), "5,3,4");
    drop(v1);
    assert_eq!(destroyed_text(), "5,3,4");
    gc.gc();
    assert_eq!(destroyed_text(), "5,3,4,2,1");
}
