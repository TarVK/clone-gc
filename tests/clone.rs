mod common;

use clone_gc::GCManager;
use common::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

#[test]
fn deep_clone() {
    const COUNT: usize = 1000;

    let gc = GCManager::new();

    let destroyed = Rc::new(RefCell::new(Vec::new()));
    let values = (0..COUNT)
        .map(|i| BiGraph::new(&gc, destroyed.clone(), i))
        .collect::<Vec<_>>();

    // Create the graph randomly
    let mut rnd = StdRng::seed_from_u64(0);
    for value in values.iter() {
        let first_index = rnd.random_range(0..COUNT);
        let second_index = rnd.random_range(0..COUNT);
        value.first.set(values.get(first_index).cloned());
        value.second.set(values.get(second_index).cloned());
    }

    // Make a copy
    let root = values.get(0).unwrap().clone();
    let start = Instant::now();
    let (gc2, root2) = gc.deep_clone(root.clone());
    println!("Deep cloned in {} ms", start.elapsed().as_millis());

    // Update all the root2 ids
    let mut reachable_count = 0;
    let mut queue = Vec::from([root2.clone()]);
    while let Some(node) = queue.pop() {
        let id = *node.id.get();
        if id >= COUNT {
            continue;
        }
        reachable_count += 1;
        node.id.set(id + COUNT);
        if let Some(child) = &*node.first.get() {
            queue.push(child.clone())
        }
        if let Some(child) = &*node.second.get() {
            queue.push(child.clone())
        }
    }
    println!("Reachable count: {reachable_count}");

    // Trace from both roots and establish they have the same structure
    let mut covered = HashSet::new();
    let mut queue = Vec::from([(root.clone(), root2.clone())]);
    while let Some((a, b)) = queue.pop() {
        if covered.contains(&(a.clone(), b.clone())) {
            continue;
        }

        assert_eq!(*a.id.get() + COUNT, *b.id.get());
        if let Some(child) = &*a.first.get() {
            queue.push((child.clone(), (*b.first.get()).as_ref().unwrap().clone()))
        }
        if let Some(child) = &*a.second.get() {
            queue.push((child.clone(), (*b.second.get()).as_ref().unwrap().clone()))
        }

        covered.insert((a, b));
    }

    // Cleanup
    drop((values, covered, root, root2));
    let gc_count = gc.gc();
    assert!(gc_count > COUNT / 2);
    let gc2_count = gc2.gc();
    assert!(gc2_count > COUNT / 2);
    println!("GCed {gc_count} and {gc2_count} items");
}
