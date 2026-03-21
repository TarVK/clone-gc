use std::{cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc};

use erased_serde::{Serialize as ErasedSerialize, Serializer as ErasedSerializer};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeSeq};

use crate::{GCP, Trace, deserialization::DynDeserializer, gc_pointer::GCPInner, root::GCPRoot};

// The main serialization entry
impl<V> GCP<V>
where
    V: Trace + Serialize,
{
    pub fn graph_serializer<'a>(&'a self) -> GraphSerializer<'a, Self, JSONDynSerializer> {
        self.graph_serializer_advanced::<JSONDynSerializer>(50)
    }
    pub fn graph_serializer_advanced<'a, D>(&'a self, depth: u32) -> GraphSerializer<'a, Self, D>
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
impl<V> Serialize for GCPRoot<V>
where
    V: Trace + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0
            .as_ref()
            .unwrap()
            .graph_serializer()
            .serialize(serializer)
    }
}

// Traits to allow multiple references to this data to be serialized only once
pub(crate) struct SerializeData {
    pub index: Option<u32>,
}
pub(crate) trait PtrSerializeData<V> {
    fn with_serialize_data<R, F: FnOnce(&mut SerializeData) -> R>(&self, f: F) -> R;
    fn get_value(&self) -> &V;
}
pub(crate) trait DynIterSerialize: ErasedSerialize {
    fn reset_serialize_data(&self);
}
pub(crate) trait PtrDynSerializeData<V>: PtrSerializeData<V> {
    fn get_dyn_serialize(&self) -> Rc<dyn DynIterSerialize>;
}

// Implementations of the graph serialization for GCP
impl<V> Serialize for GCPInner<V>
where
    V: Trace + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        dyn_serialize(self, serializer)
    }
}
impl<V> Serialize for GCP<V>
where
    V: Trace + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        rec_serialize(self, serializer)
    }
}

impl<V> PtrSerializeData<V> for GCP<V>
where
    V: Trace,
{
    fn with_serialize_data<R, F: FnOnce(&mut SerializeData) -> R>(&self, f: F) -> R {
        f(&mut *self.0.serialize_data.borrow_mut())
    }
    fn get_value(&self) -> &V {
        &self.0.value.as_ref().expect("Cannot serialize after GCed")
    }
}
impl<V> PtrDynSerializeData<V> for GCP<V>
where
    V: Trace + Serialize,
{
    fn get_dyn_serialize(&self) -> Rc<dyn DynIterSerialize> {
        self.0.clone()
    }
}

impl<V> PtrSerializeData<V> for GCPInner<V>
where
    V: Trace,
{
    fn with_serialize_data<R, F: FnOnce(&mut SerializeData) -> R>(&self, f: F) -> R {
        f(&mut *self.serialize_data.borrow_mut())
    }
    fn get_value(&self) -> &V {
        &self.value.as_ref().expect("Cannot serialize after GCed")
    }
}
impl<V> DynIterSerialize for GCPInner<V>
where
    V: Trace + Serialize,
{
    fn reset_serialize_data(&self) {
        self.with_serialize_data(|data| data.index = None);
    }
}

// The serialization logic
#[derive(Default)]
pub struct GraphSerializeState {
    active: bool,
    queue: VecDeque<(ID, Rc<dyn DynIterSerialize>)>,
    found: Vec<Rc<dyn DynIterSerialize>>,
    index: ID,
    /// The amount of recursion remaining
    remaining_rec_depth: u32,
}
pub type ID = u32;
thread_local! {
    static SERIALIZE_STATE: RefCell<GraphSerializeState> = RefCell::new(Default::default());
}

#[derive(Serialize)]
#[serde(untagged, bound(serialize = "V: Serialize"))]
// #[serde(bound(serialize = "V: Serialize"))]
pub enum GCPSerializationData<'a, V> {
    Ptr { ptr: ID },
    Data { ptr: ID, value: &'a V },
}

pub(crate) fn dyn_serialize<S, V, D>(data: &D, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Serialize,
    D: PtrSerializeData<V>,
{
    let id = data.with_serialize_data(|data| data.index.unwrap());

    SERIALIZE_STATE.with_borrow_mut(|state| {
        state.remaining_rec_depth -= 1;
    });

    let data = GCPSerializationData::Data {
        ptr: id,
        value: data.get_value(),
    };
    let res = data.serialize(serializer);

    SERIALIZE_STATE.with_borrow_mut(|state| {
        state.remaining_rec_depth += 1;
    });

    res
}

pub(crate) fn rec_serialize<S, V, D>(data: &D, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Serialize,
    D: PtrDynSerializeData<V>,
{
    let (id, rec) = data.with_serialize_data(|serialize_data| {
        if let Some(index) = serialize_data.index {
            return (index, false);
        }

        SERIALIZE_STATE.with_borrow_mut(|state| {
            if !state.active {
                panic!("Serialization may only be initiated through GCManager.serialize");
            }
            let id = state.index;
            state.index = id + 1;
            serialize_data.index = Some(id);

            let inner = data.get_dyn_serialize();
            let recurse = state.remaining_rec_depth > 0;
            if recurse {
                state.found.push(inner);
                state.remaining_rec_depth -= 1;
                (id, true)
            } else {
                state.queue.push_back((id, inner));
                (id, false)
            }
        })
    });

    if rec {
        let data = GCPSerializationData::Data {
            ptr: id,
            value: data.get_value(),
        };
        let res = data.serialize(serializer);

        SERIALIZE_STATE.with_borrow_mut(|state| {
            state.remaining_rec_depth += 1;
        });
        res
    } else {
        let ptr: GCPSerializationData<V> = GCPSerializationData::Ptr { ptr: id };
        ptr.serialize(serializer)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "R: Serialize, D: DynSerializer",
    deserialize = "R: Deserialize<'de>, D: DynDeserializer<'de> + 'de"
))]
pub struct RootedSerialize<R, D> {
    pub(crate) root: R,
    pub(crate) support: SupportSerializer<D>,
}

pub struct SupportSerializer<D> {
    pub(crate) dyn_serializer: PhantomData<D>,
    pub(crate) depth: u32,
}
#[derive(Serialize, Deserialize)]
pub struct Support<S>(pub(crate) Vec<(u32, S)>);
impl<D: DynSerializer> Serialize for SupportSerializer<D> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut support = Vec::new();
        loop {
            let first = SERIALIZE_STATE.with_borrow_mut(|state| {
                state.remaining_rec_depth = self.depth;
                state.queue.pop_front()
            });
            let Some((id, dyn_serialize)) = first else {
                break;
            };

            let serializable = D::serialize_dyn(&*dyn_serialize);
            support.push((id, serializable));

            SERIALIZE_STATE.with_borrow_mut(|state| {
                state.found.push(dyn_serialize);
            });
        }
        Support(support).serialize(serializer)
    }
}

pub struct GraphSerializer<'a, R, D> {
    pub(crate) dyn_serializer: PhantomData<D>,
    pub(crate) root: &'a R,
    pub(crate) depth: u32,
}
impl<'a, R, D> Serialize for GraphSerializer<'a, R, D>
where
    R: Serialize,
    D: DynSerializer + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SERIALIZE_STATE.with_borrow_mut(|state| {
            *state = GraphSerializeState {
                active: true,
                queue: VecDeque::new(),
                found: Vec::new(),
                index: 0,
                remaining_rec_depth: self.depth,
            };
        });

        let r = RootedSerialize {
            root: self.root,
            support: SupportSerializer {
                dyn_serializer: self.dyn_serializer.clone(),
                depth: self.depth,
            },
        };
        let res = r.serialize(serializer);

        SERIALIZE_STATE.with_borrow_mut(|state| {
            for d in &state.found {
                d.reset_serialize_data();
            }
            state.found.clear();
            state.active = false;
            assert_eq!(state.queue.len(), 0);
        });

        res
    }
}

// A dynamic serializer to optionally break away from recursion and prevent stack-overflows
pub trait DynSerializer {
    type S: Serialize;
    fn serialize_dyn(data: &dyn ErasedSerialize) -> Self::S;
}

#[derive(Clone)]
pub struct JSONDynSerializer;
impl DynSerializer for JSONDynSerializer {
    type S = String;
    fn serialize_dyn(data: &dyn ErasedSerialize) -> Self::S {
        let mut out = Vec::new();
        let json_serializer = &mut serde_json::Serializer::new(&mut out);
        let mut erased = <dyn ErasedSerializer>::erase(json_serializer);
        data.erased_serialize(&mut erased);

        String::from_utf8(out).unwrap()
    }
}
