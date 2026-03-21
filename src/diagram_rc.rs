use std::{
    cell::RefCell,
    fmt::{Debug, Display},
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
    rc::{Rc, Weak},
};

use serde::{Serialize, Serializer};

use crate::{
    DynSerializer, JSONDynSerializer,
    serialization::{
        DynIterSerialize, GraphSerializer, PtrDynSerializeData, PtrSerializeData, SerializeData,
        dyn_serialize, rec_serialize,
    },
};

// A diagram rc implementation that uses diagram/graph serialization
pub struct DRc<V>(Rc<DRcInner<V>>);
pub struct DRcInner<V> {
    serialize_data: RefCell<SerializeData>,
    value: V,
}
impl<V> DRc<V> {
    pub fn new(val: V) -> Self {
        DRc(Rc::new(DRcInner {
            serialize_data: RefCell::new(SerializeData { index: None }),
            value: val,
        }))
    }
    pub fn downgrade(val: &Self) -> DWeak<V> {
        DWeak(Rc::downgrade(&val.0))
    }
    pub fn strong_count(val: &Self) -> usize {
        Rc::strong_count(&val.0)
    }
    pub fn weak_count(val: &Self) -> usize {
        Rc::weak_count(&val.0)
    }
}
impl<V> DRc<V>
where
    V: Serialize + 'static,
{
    pub fn graph_serializer(&self) -> GraphSerializer<Self, JSONDynSerializer> {
        self.graph_serializer_advanced::<JSONDynSerializer>(50)
    }
    pub fn graph_serializer_advanced<D>(&self, depth: u32) -> GraphSerializer<Self, D>
    where
        D: DynSerializer + Clone,
    {
        GraphSerializer {
            dyn_serializer: PhantomData,
            root: self,
            depth,
        }
    }
}
impl<V> Deref for DRc<V> {
    type Target = V;
    fn deref(&self) -> &Self::Target {
        &(*self.0).value
    }
}
impl<V> AsRef<V> for DRc<V> {
    fn as_ref(&self) -> &V {
        &(*self.0).value
    }
}
impl<V> Clone for DRc<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<V> PartialEq for DRc<V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.value == other.0.value
    }
}
impl<V> Eq for DRc<V> where V: Eq {}
impl<V> PartialOrd for DRc<V>
where
    V: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.value.partial_cmp(&other.0.value)
    }
}
impl<V> Ord for DRc<V>
where
    V: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.value.cmp(&other.0.value)
    }
}
impl<V> Hash for DRc<V>
where
    V: Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.value.hash(state);
    }
}
impl<V> Debug for DRc<V>
where
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DRc").field(&self.0.value).finish()
    }
}
impl<V> Display for DRc<V>
where
    V: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.value.fmt(f)
    }
}
impl<V> From<V> for DRc<V> {
    fn from(value: V) -> Self {
        DRc::new(value)
    }
}

// Implementation of serialization
impl<V> Serialize for DRcInner<V>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        dyn_serialize(self, serializer)
    }
}
impl<V> Serialize for DRc<V>
where
    V: Serialize + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        rec_serialize(self, serializer)
    }
}

impl<V> PtrSerializeData<V> for DRc<V> {
    fn with_serialize_data<R, F: FnOnce(&mut SerializeData) -> R>(&self, f: F) -> R {
        f(&mut *self.0.serialize_data.borrow_mut())
    }
    fn get_value(&self) -> &V {
        &self.0.value
    }
}
impl<V> PtrDynSerializeData<V> for DRc<V>
where
    V: Serialize + 'static,
{
    fn get_dyn_serialize(&self) -> Rc<dyn DynIterSerialize> {
        self.0.clone()
    }
}

impl<V> PtrSerializeData<V> for DRcInner<V> {
    fn with_serialize_data<R, F: FnOnce(&mut SerializeData) -> R>(&self, f: F) -> R {
        f(&mut *self.serialize_data.borrow_mut())
    }
    fn get_value(&self) -> &V {
        &self.value
    }
}
impl<V> DynIterSerialize for DRcInner<V>
where
    V: Serialize,
{
    fn reset_serialize_data(&self) {
        self.with_serialize_data(|data| data.index = None);
    }
}

// A weak rc implementation that uses diagram/graph serialization
pub struct DWeak<V>(Weak<DRcInner<V>>);
impl<V> DWeak<V> {
    pub fn upgrade(&self) -> Option<DRc<V>> {
        self.0.upgrade().map(|v| DRc(v))
    }
    pub fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
    pub fn weak_count(&self) -> usize {
        self.0.weak_count()
    }
}
impl<V> Clone for DWeak<V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<V> Debug for DWeak<V>
where
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
