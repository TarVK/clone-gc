use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use crate::{
    GCManager, GCP, GetGCManager, Trace, gc_pointer::GCPInner, root::GCPRoot,
    weak_gc_pointer::DynGCP,
};

// The data stored on the GCP to allow for cloning (and serialization)
pub(crate) struct CloneData<V: Trace + 'static> {
    pub clone: Option<Rc<GCPInner<V>>>,
}

// The graph cloning trait + implementation
pub struct GraphCloneState {
    manager: GCManager,
    queue: Vec<Rc<dyn CloneInternalValue>>,
}
pub trait GraphClone {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self;
}
impl<V: Trace + GraphClone + 'static> GraphClone for GCP<V> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        let mut clone_data = self.clone_data();
        if let Some(ref clone) = clone_data.clone {
            assert!((*clone).meta().gc == m.manager); // Otherwise the previous deep_clone did not cleanup after itself.
            return GCP(clone.clone());
        }

        let ptr = GCP::<V>::new_raw(&m.manager, None);
        clone_data.clone = Some(ptr.clone());

        m.queue.push(self.0.clone());
        return GCP(ptr);
    }
}

// A separate trait to set the internal value, to allow for iterative cloning
trait CloneInternalValue {
    fn clone_internal(&self, m: &mut GraphCloneState);
    fn reset_clone_data(&self);
}
impl<V: Trace + GraphClone + 'static> CloneInternalValue for GCPInner<V> {
    fn clone_internal(&self, m: &mut GraphCloneState) {
        let Some(value) = &self.value else {
            return;
        };
        let cloned = value.graph_clone(m);

        let clone_data = self.clone_data.borrow_mut();
        let clone_inner = &**clone_data.clone.as_ref().unwrap();
        clone_inner.set_value(cloned);
    }
    fn reset_clone_data(&self) {
        self.clone_data.borrow_mut().clone = None
    }
}
impl<V> GCPInner<V>
where
    V: Trace,
{
    pub(crate) fn set_value(&self, val: V) {
        assert!(if let None = self.value { true } else { false });
        unsafe {
            let ptr = self as *const GCPInner<V> as *mut GCPInner<V>;
            (*ptr).value = Some(val);
        }
    }
}

// The cloning orchestrator
impl GCManager {
    pub fn deep_clone<V: Trace + GraphClone + 'static, K: Into<GCP<V>> + From<GCP<V>>>(
        &self,
        root: K,
    ) -> (GCManager, K) {
        let mut state = GraphCloneState {
            manager: GCManager::new(),
            queue: Vec::new(),
        };
        let root: GCP<V> = root.into();
        let out = root.graph_clone(&mut state);

        // Perform iterative clone of internal structure
        let mut cloned = Vec::new();
        while let Some(cloneable) = state.queue.pop() {
            (&*cloneable).clone_internal(&mut state);
            cloned.push(cloneable);
        }

        // Remove cloning data
        for cloneable in cloned {
            (&*cloneable).reset_clone_data();
        }

        (state.manager, K::from(out))
    }
}

// Deep cloning is the default for a root
impl<V> Clone for GCPRoot<V>
where
    V: Trace + GraphClone,
{
    fn clone(&self) -> Self {
        let manager = self.get_manager();
        let (_, root_clone) = manager.deep_clone(self.0.clone().unwrap());
        GCPRoot(Some(root_clone))
    }
}

// Implementations
impl<X: GraphClone> GraphClone for RefCell<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        RefCell::new(self.borrow().graph_clone(m))
    }
}
impl<X: GraphClone> GraphClone for Option<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        match self {
            Some(v) => Some(v.graph_clone(m)),
            None => None,
        }
    }
}
impl<X: GraphClone> GraphClone for Vec<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        self.iter().map(|v| v.graph_clone(m)).collect()
    }
}
impl<X: GraphClone + Eq + Hash> GraphClone for HashSet<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        self.iter().map(|v| v.graph_clone(m)).collect()
    }
}
impl<X: GraphClone + Eq + Hash, Y: GraphClone> GraphClone for HashMap<X, Y> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        self.iter()
            .map(|(k, v)| (k.graph_clone(m), v.graph_clone(m)))
            .collect()
    }
}

macro_rules! impl_graph_clone {
    ($type:tt) => {
        impl GraphClone for $type {
            fn graph_clone(&self, _m: &mut GraphCloneState) -> Self {
                self.clone()
            }
        }
    };
}
impl_graph_clone!(bool);
impl_graph_clone!(f32);
impl_graph_clone!(f64);
impl_graph_clone!(u8);
impl_graph_clone!(u16);
impl_graph_clone!(u32);
impl_graph_clone!(u64);
impl_graph_clone!(usize);
impl_graph_clone!(i8);
impl_graph_clone!(i16);
impl_graph_clone!(i32);
impl_graph_clone!(i64);
impl_graph_clone!(isize);
impl_graph_clone!(String);

// Rc is cloned shallowly
impl<X> GraphClone for Rc<X> {
    fn graph_clone(&self, _m: &mut GraphCloneState) -> Self {
        self.clone()
    }
}
