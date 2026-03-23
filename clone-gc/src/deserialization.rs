use std::{any::Any, cell::RefCell, collections::HashMap, marker::PhantomData, rc::Rc};

use erased_serde::Deserializer as ErasedDeserializer;
use serde::{Deserialize, Deserializer, de::DeserializeOwned, de::Error};

use crate::{
    GCManager, GCP, JSONDynSerializer, Trace,
    root::GCPRoot,
    serialization::{
        GCPSerializationData, GraphSerializer, ID, RootedSerialize, Support, SupportSerializer,
    },
};

// The main deserialization entries
impl<'de, V> GCP<V>
where
    V: Trace + DeserializeOwned,
{
    pub fn graph_deserializer() -> GraphDeserializeData<Self, JSONDynSerializer> {
        Self::graph_deserializer_advanced::<JSONDynSerializer>()
    }
    pub fn graph_deserializer_advanced<D>() -> GraphDeserializeData<Self, D>
    where
        D: DynDeserializer<'de> + 'de,
    {
        GraphDeserializeData(PhantomData)
    }
}
impl<'de, V> Deserialize<'de> for GCPRoot<V>
where
    V: Trace + DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<GCPRoot<V>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(GCPRoot(Some(
            GCP::<V>::graph_deserializer().deserialize(deserializer)?,
        )))
    }
}

pub struct GraphDeserializeData<V, DD>(PhantomData<(V, DD)>);
impl<'de, V, DD> GraphDeserializeData<V, DD>
where
    V: Deserialize<'de>,
    DD: DynDeserializer<'de> + 'de,
{
    fn deserialize<D>(&self, deserializer: D) -> Result<V, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let p = GraphDeserializer::<V, DD>::deserialize(deserializer)?;
        Ok(p.root)
    }
}

// Traits to allow multiple references to the same data to be created
pub trait PtrDeserializeData: DeserializeOwned {
    type V: DeserializeOwned;
    fn create_container(manager: &GCManager) -> Self;
    fn set_value(&self, value: Self::V);
}

// Implementations of the graph deserialization for GCP
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

// The deserialization logic
#[derive(Default)]
pub struct GraphDeserializeState {
    manager: Option<GCManager>,
    support_deserializers: HashMap<ID, DynTypeDeserializer>,
    nodes: HashMap<ID, Rc<dyn Any>>,
}
thread_local! {
    static DESERIALIZE_STATE: RefCell<GraphDeserializeState> = RefCell::new(Default::default());
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
