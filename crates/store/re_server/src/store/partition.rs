use std::collections::{HashMap, hash_map::Entry};

use itertools::Itertools as _;

use re_chunk_store::ChunkStoreHandle;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;

use crate::store::{Error, Layer};

#[derive(Clone)]
pub struct Partition {
    /// The layers of this partition.
    layers: HashMap<String, Layer>,

    last_updated_at: jiff::Timestamp,
}

impl Default for Partition {
    fn default() -> Self {
        Self {
            layers: HashMap::default(),
            last_updated_at: jiff::Timestamp::now(),
        }
    }
}

impl Partition {
    pub fn from_layer_data(layer_name: &str, chunk_store_handle: ChunkStoreHandle) -> Self {
        Self {
            layers: vec![(layer_name.to_owned(), Layer::new(chunk_store_handle))]
                .into_iter()
                .collect(),
            last_updated_at: jiff::Timestamp::now(),
        }
    }

    /// Iterate over layers.
    ///
    /// Layers are iterated in (registration time, layer name) order, as per how they should appear
    /// in the partition table.
    pub fn iter_layers(&self) -> impl Iterator<Item = (&str, &Layer)> {
        self.layers
            .iter()
            .sorted_by(|(name_a, layer_a), (name_b, layer_b)| {
                (layer_a.registration_time(), name_a).cmp(&(layer_b.registration_time(), name_b))
            })
            .map(|(layer_name, layer)| (layer_name.as_str(), layer))
    }

    pub fn layer(&self, layer_name: &str) -> Option<&Layer> {
        self.layers.get(layer_name)
    }

    pub fn last_updated_at(&self) -> jiff::Timestamp {
        self.last_updated_at
    }

    pub fn insert_layer(
        &mut self,
        layer_name: String,
        layer: Layer,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        match self.layers.entry(layer_name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(layer);
                self.last_updated_at = jiff::Timestamp::now();
            }

            Entry::Occupied(mut entry) => match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    entry.insert(layer);
                    self.last_updated_at = jiff::Timestamp::now();
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring layer '{layer_name}': already exists in partition");
                }
                IfDuplicateBehavior::Error => {
                    return Err(Error::LayerAlreadyExists(layer_name));
                }
            },
        }

        Ok(())
    }

    pub fn num_chunks(&self) -> u64 {
        self.layers.values().map(|layer| layer.num_chunks()).sum()
    }

    pub fn size_bytes(&self) -> u64 {
        self.layers.values().map(|layer| layer.size_bytes()).sum()
    }
}
