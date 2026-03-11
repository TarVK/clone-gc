use std::{cell::RefCell, rc::Rc};

use crate::{GCManager, GCP, Trace, gc_pointer::GCPInner, weak_gc_pointer::DynGCP};

// The data stored on the GCP to allow for cloning
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
        unsafe {
            let ptr = clone_inner as *const GCPInner<V> as *mut GCPInner<V>;
            (*ptr).value = Some(cloned);
        }
    }
    fn reset_clone_data(&self) {
        self.clone_data.borrow_mut().clone = None
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

// Implementations
impl<X: GraphClone> GraphClone for RefCell<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        RefCell::new(self.borrow().graph_clone(m))
    }
}
impl<X: GraphClone> GraphClone for Rc<X> {
    fn graph_clone(&self, m: &mut GraphCloneState) -> Self {
        let val: &X = &*self;
        Rc::new(val.graph_clone(m))
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
