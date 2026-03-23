mod common;
use std::{cell::RefCell, rc::Rc, time::Instant};

use clone_gc::{DRc, Field, GCManager, GCP, GraphDeserializer, JSONDynSerializer};
use common::*;
use serde::{Deserialize, Serialize};

#[test]
pub fn serialize_ll() {
    serialize_ll_to_string();
}
const SIZE: usize = 1000;
pub fn serialize_ll_to_string() -> String {
    let drop_count = Rc::new(RefCell::new(0));
    let gc = GCManager::new();
    let end = GCP::new(&gc, GCLLInner::End(RefCell::new(None)));
    let ll = (0..SIZE).fold(end.clone(), |n, _i| {
        GCP::new(&gc, GCLLInner::Next(n, drop_count.clone()))
    });
    match &*end {
        GCLLInner::Next(_, _) => {}
        GCLLInner::End(ref_cell) => *ref_cell.borrow_mut() = Some(ll.clone()),
    };

    let start = Instant::now();
    // for i in 0..10 {
    let out =
        serde_json::to_string(&ll.graph_serializer_advanced::<JSONDynSerializer>(30)).unwrap();
    // }
    println!("Passed {} ms serializing", start.elapsed().as_millis());

    println!("Out: {}", out);
    out
}

#[test]
pub fn deserialize_ll() {
    let text = serialize_ll_to_string();
    let start = Instant::now();
    let v: GraphDeserializer<GCP<GCLLInner>, JSONDynSerializer> =
        serde_json::from_str(&text).expect("Deserializes");
    println!("Passed {} ms deserializing", start.elapsed().as_millis());
    let root = v.root;
    let mut node = root.clone();

    let mut count = 0;
    loop {
        match &*node {
            GCLLInner::Next(gcp, _) => {
                node = gcp.clone();
                count += 1;
            }
            GCLLInner::End(p) => {
                assert!(p.borrow().as_ref().unwrap() == &root);
                break;
            }
        }
    }
    assert_eq!(count, SIZE);
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(untagged)]
enum DAGInner {
    Sync(u32),
    Node(DAG, DAG),
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct DAG(DRc<DAGInner>);
impl DAG {
    fn sync(val: u32) -> Self {
        Self(DRc::new(DAGInner::Sync(val)))
    }
    fn node(left: DAG, right: DAG) -> Self {
        Self(DRc::new(DAGInner::Node(left, right)))
    }
}

#[test]
pub fn serialize_rc() {
    serialize_rc_string(1);
}
pub fn serialize_rc_string(v1: u32) -> (DAG, String) {
    let n1 = DAG::sync(v1);
    let n2 = DAG::sync(2);
    let n3 = DAG::sync(3);
    let n4 = DAG::node(n1.clone(), n2.clone());
    let n5 = DAG::node(n2.clone(), n3);
    let n6 = DAG::node(n5, n2);
    let n7 = DAG::node(n4, n6);
    let n8 = DAG::node(n7, n1);

    let out = serde_json::to_string_pretty(&n8.0.graph_serializer()).unwrap();
    println!("DAG: {}", out);
    (n8, out)
}

#[test]
pub fn deserialize_rc() {
    let (dag_src, dag_text) = serialize_rc_string(1);
    let (dag_src2, _) = serialize_rc_string(2);

    let dag: GraphDeserializer<DAG, JSONDynSerializer> =
        serde_json::from_str(&dag_text).expect("Deserializes");
    let dag = dag.root;

    assert_eq!(dag_src, dag);
    assert_ne!(dag_src2, dag);
}
