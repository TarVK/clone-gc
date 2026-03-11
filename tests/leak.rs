use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

use rand::{RngExt, SeedableRng, rngs::StdRng};

use clone_gc::GCManager;
mod common;
use common::*;

#[test]
fn leak() {
    const COUNT: usize = 1000;

    let gc = GCManager::new();

    let destroyed = Rc::new(RefCell::new(Vec::new()));
    let values = (0..COUNT * 2)
        .map(|i| BiGraph::new(&gc, destroyed.clone(), i))
        .collect::<Vec<_>>();

    // Create the graph randomly, but ensure the two halves are split
    let mut rnd = StdRng::seed_from_u64(0);
    for (index, value) in values.iter().enumerate() {
        let mut first_index = rnd.random_range(0..COUNT);
        let mut second_index = rnd.random_range(0..COUNT);
        if index >= COUNT {
            first_index += COUNT;
            second_index += COUNT;
        }
        value.first.set(values.get(first_index).cloned());
        value.second.set(values.get(second_index).cloned());
    }

    // Keep everything reachable from a single root
    let root = values.first().unwrap().clone();
    drop(values);
    let start = Instant::now();
    gc.gc();
    println!("Passed {}", start.elapsed().as_millis());
    assert!(
        destroyed.borrow().len() >= COUNT,
        "at least half of the items should be destroyed"
    );

    let destroyed_set = destroyed.borrow().iter().cloned().collect::<HashSet<_>>();
    for id in COUNT..COUNT * 2 {
        assert!(destroyed_set.contains(&id), "{id} must be destroyed");
    }
    println!("Destroyed: {}", destroyed.borrow().len());

    // Check if we can safely follow pointers from the root
    let mut found = HashSet::new();
    let mut stack = Vec::from([root.clone()]);
    while let Some(node) = stack.pop() {
        if found.contains(&node.id.get()) {
            continue;
        }
        found.insert(node.id.get());
        stack.push((*node.first.get()).clone().unwrap());
        stack.push((*node.second.get()).clone().unwrap());
    }
    assert!(
        found.len() > COUNT / 2,
        "We will probably find at least a quarter of the nodes to still be reachable (though this is no guarantee from spec)"
    );

    // All data should be dropped when dropping the root
    drop(root);
    let start = Instant::now();
    gc.gc();
    println!("Passed {}", start.elapsed().as_millis());
    println!("Destroyed {}", destroyed.borrow().len());
    assert!(
        destroyed.borrow().len() >= COUNT * 2,
        "all the items should be destroyed"
    );
    let destroyed_set = destroyed.borrow().iter().cloned().collect::<HashSet<_>>();
    for id in 0..COUNT * 2 {
        assert!(destroyed_set.contains(&id), "{id} must be destroyed");
    }
}
