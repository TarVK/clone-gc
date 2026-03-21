use std::ops::Deref;

use crate::{GCManager, GCP, GetGCManager, Trace};

/// A rooted GCP which:
/// - performs deep clones on clone (see clone.rs)
/// - performs garbage collection on drop, cleaning all reachable state iff it is only reachable from this root
/// - performs serialization from this root (see serialization.rs)
pub struct GCPRoot<V: Trace + 'static>(pub(crate) Option<GCP<V>>);
impl<V> GCPRoot<V>
where
    V: Trace,
{
    pub fn new(val: V) -> Self {
        let manager = GCManager::new();
        Self(Some(GCP::new(&manager, val)))
    }
}
impl<V> GetGCManager for GCPRoot<V>
where
    V: Trace,
{
    fn get_manager(&self) -> GCManager {
        self.0.as_ref().unwrap().get_manager()
    }
}
impl<V> Deref for GCPRoot<V>
where
    V: Trace,
{
    type Target = GCP<V>;
    fn deref(&self) -> &Self::Target {
        &self.0.as_ref().unwrap()
    }
}
impl<V> Drop for GCPRoot<V>
where
    V: Trace,
{
    fn drop(&mut self) {
        let manager = self.get_manager();
        drop(self.0.take());
        manager.gc();
    }
}
