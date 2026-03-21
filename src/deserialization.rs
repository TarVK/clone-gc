use std::{any::Any, cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

use erased_serde::Deserializer as ErasedDeserializer;
use serde::{Deserialize, Deserializer, de::DeserializeOwned, de::Error};

use crate::{
    GCManager, GCP, JSONDynSerializer, Trace,
    serialization::{
        GCPSerializationData, GraphSerializer, ID, RootedSerialize, Support, SupportSerializer,
    },
};

// The main deserialization entry
// impl<'de, V> GCP<V>
// where
//     V: Trace + Deserialize<'de>,
// {
//     pub fn graph_deserializer() -> GraphDeserializer<JSONDynDeserializer> {
//         self.graph_deserializer_advanced(JSONDynDeserializer)
//     }
//     pub fn graph_deserializer_advanced<D>(dyn_serializer: D) -> GraphSerializer<D>
//     where
//         D: DynDeserializer + Clone,
//     {
//         GraphDeserializer { dyn_serializer }
//     }
// }
// impl<V> Serialize for GCPRoot<V>
// where
//     V: Trace + Serialize,
// {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         self.0
//             .as_ref()
//             .unwrap()
//             .graph_serializer()
//             .serialize(serializer)
//     }
// }

// The deserialization logic
#[derive(Default)]
pub struct GraphDeserializeState {
    manager: Option<GCManager>,
    support_deserializers: HashMap<u32, DynTypeDeserializer>,
    nodes: HashMap<u32, Rc<dyn Any>>,
}
pub struct DynTypeDeserializer(
    Box<dyn FnOnce(&mut dyn ErasedDeserializer) -> Result<(), erased_serde::Error>>,
);
impl GraphDeserializeState {
    pub fn get_or_create_node<'de, V: PtrDeserializeData + 'static>(
        &mut self,
        id: ID,
    ) -> Option<Rc<V>> {
        let manager = &self
            .manager
            .as_ref()
            .expect("Deserialization is only allowed from GraphDeserializer or GCPRoot");
        let node = self
            .nodes
            .entry(id)
            .or_insert_with(|| Rc::new(V::create_container(manager)));
        node.clone().downcast().ok()
    }
}
pub trait PtrDeserializeData: DeserializeOwned {
    type V: DeserializeOwned;
    fn create_container(manager: &GCManager) -> Self;
    fn set_value(&self, value: Self::V);
}

impl<'de, V> Deserialize<'de> for GCP<V>
where
    V: Trace + DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        rec_deserialize(deserializer)
    }
}
impl<V> PtrDeserializeData for GCP<V>
where
    V: Trace + DeserializeOwned,
{
    type V = V;
    fn create_container(manager: &GCManager) -> Self {
        GCP(GCP::new_raw(manager, None))
    }
    fn set_value(&self, value: Self::V) {
        self.0.set_value(value);
    }
}
// pub(crate) fn dyn_deserialize<'de, S, V, D>(data: &D, deserializer: S) -> Result<S::Ok, S::Error>
// where
//     S: Deserializer,
//     V: Deserialize<'de>,
//     D: IterSerializeData<V>,
// {
//     let id = data.with_serialize_data(|data| data.index.unwrap());

//     DESERIALIZE_STATE.with_borrow_mut(|state| {
//         state.remaining_rec_depth -= 1;
//     });

//     let data = GCPSerializationData::Data {
//         ptr: id,
//         value: data.get_value(),
//     };
//     let res = data.serialize(serializer);

//     DESERIALIZE_STATE.with_borrow_mut(|state| {
//         state.remaining_rec_depth += 1;
//     });

//     res
// }

#[derive(Deserialize)]
#[serde(untagged)]
#[serde(deny_unknown_fields)]
enum GCPDeserializationData<V> {
    Ptr { ptr: ID },
    Data { ptr: ID, value: V },
}

pub(crate) fn rec_deserialize<'de, S, D>(deserializer: S) -> Result<D, S::Error>
where
    S: Deserializer<'de>,
    D: PtrDeserializeData + Clone + 'static,
{
    let data = GCPDeserializationData::<D::V>::deserialize(deserializer)?;
    let ptr = match data {
        GCPDeserializationData::Ptr { ptr } => ptr,
        GCPDeserializationData::Data { ptr, value: _ } => ptr,
    };
    let Some(container) =
        DESERIALIZE_STATE.with_borrow_mut(|state| state.get_or_create_node::<D>(ptr))
    else {
        return Err(S::Error::custom(format!(
            "Value with id {ptr} is not of the correct type"
        )));
    };
    let container_clone = (*container).clone();

    // Deserialize the value, or store how to deserialize it in the future
    match data {
        GCPDeserializationData::Data { ptr: _, value } => container.set_value(value),
        GCPDeserializationData::Ptr { ptr } => {
            DESERIALIZE_STATE.with_borrow_mut(|state| {
                state
                    .support_deserializers
                    .entry(ptr)
                    .or_insert_with(move || {
                        DynTypeDeserializer(Box::new(move |d| {
                            // Try again, with the actual data
                            let _: D = erased_serde::deserialize(d)?;
                            Ok(())
                        }))
                    });
            });
        }
    }

    Ok(container_clone)
}

// impl<'de, DD> Deserialize<'de> for DynTypeDeserializer<DD>
// where
//     DD: Deserialize<'de>,
// {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         DD::deserialize(deserializer)?;
//         Ok(DynTypeDeserializer(PhantomData))
//     }
// }
// trait DynTypeDeserializer {
//     fn deserialize(deserializer: &dyn Deserializer) -> ();
// }

thread_local! {
    static DESERIALIZE_STATE: RefCell<GraphDeserializeState> = RefCell::new(Default::default());
}

impl<'de, DD> Deserialize<'de> for SupportSerializer<DD>
where
    DD: DynDeserializer<'de> + 'de,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let support = Support::<DD::D>::deserialize(deserializer)?;
        for (id, data) in support.0 {
            let deserialize = DESERIALIZE_STATE.with_borrow_mut(|state| {
                let Some(deserialize) = state.support_deserializers.remove(&id) else {
                    return Err(D::Error::custom(format!(
                        "No data with ptr {id} is reachable from the root."
                    )));
                };
                Ok(deserialize)
            })?;
            DD::deserialize(data, deserialize).map_err(|e| D::Error::custom(e))?;
        }

        // Return dummy data
        Ok(SupportSerializer {
            dyn_serializer: PhantomData,
            depth: 0,
        })
    }
}

// pub struct GraphDeserialize<R> {
//     root: PhantomData<R>,
// }
pub struct GraphDeserializer<R, DD> {
    pub(crate) dyn_deserializer: PhantomData<DD>,
    pub root: R,
}
impl<'de, R, DD> Deserialize<'de> for GraphDeserializer<R, DD>
where
    R: Deserialize<'de>,
    DD: DynDeserializer<'de> + 'de,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        DESERIALIZE_STATE.with_borrow_mut(|state| {
            *state = GraphDeserializeState {
                manager: Some(GCManager::new()),
                support_deserializers: HashMap::new(),
                nodes: HashMap::new(),
            }
        });

        let res = RootedSerialize::<R, DD>::deserialize(deserializer);

        DESERIALIZE_STATE.with_borrow_mut(|state| {
            state.manager = None;
            state.support_deserializers.clear();
        });

        let res = res?;
        Ok(GraphDeserializer {
            dyn_deserializer: PhantomData,
            root: res.root,
        })
    }
}

// A dynamic deserializer to optionally break away from recursion and prevent stack-overflows
pub trait DynDeserializer<'de> {
    type D: Deserialize<'de>;
    fn deserialize(
        data: Self::D,
        deserialize: DynTypeDeserializer,
    ) -> Result<(), erased_serde::Error>;
}

impl<'de> DynDeserializer<'de> for JSONDynSerializer {
    type D = String;
    fn deserialize(
        data: Self::D,
        deserialize: DynTypeDeserializer,
    ) -> Result<(), erased_serde::Error> {
        let mut json = serde_json::Deserializer::from_str(&data);
        let json_ref = &mut json;
        let mut deserializer: Box<dyn ErasedDeserializer> =
            Box::new(<dyn ErasedDeserializer>::erase(json_ref));
        (deserialize.0)(&mut deserializer)
    }
}
