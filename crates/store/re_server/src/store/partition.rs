use std::collections::HashMap;

use itertools::Itertools as _;
use re_chunk_store::ChunkStoreHandle;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;

use crate::store::{Error, Layer, Tracked};

/// The mutable inner state of a [`Partition`], wrapped in [`Tracked`] for automatic timestamp updates.
#[derive(Clone)]
pub struct PartitionInner {
    /// The layers of this partition.
    layers: HashMap<String, Layer>,
}

#[derive(Clone)]
pub struct Partition {
    inner: Tracked<PartitionInner>,
}

impl Default for Partition {
    fn default() -> Self {
        Self {
            inner: Tracked::new(PartitionInner {
                layers: HashMap::default(),
            }),
        }
    }
}

impl Partition {
    pub fn from_layer_data(layer_name: &str, chunk_store_handle: ChunkStoreHandle) -> Self {
        Self {
            inner: Tracked::new(PartitionInner {
                layers: vec![(layer_name.to_owned(), Layer::new(chunk_store_handle))]
                    .into_iter()
                    .collect(),
            }),
        }
    }

    pub fn layer_count(&self) -> usize {
        self.inner.layers.len()
    }

    /// Iterate over layers.
    ///
    /// Layers are iterated in (registration time, layer name) order, as per how they should appear
    /// in the partition table.
    pub fn iter_layers(&self) -> impl Iterator<Item = (&str, &Layer)> {
        self.inner
            .layers
            .iter()
            .sorted_by(|(name_a, layer_a), (name_b, layer_b)| {
                (layer_a.registration_time(), name_a).cmp(&(layer_b.registration_time(), name_b))
            })
            .map(|(layer_name, layer)| (layer_name.as_str(), layer))
    }

    pub fn layer(&self, layer_name: &str) -> Option<&Layer> {
        self.inner.layers.get(layer_name)
    }

    pub fn last_updated_at(&self) -> jiff::Timestamp {
        self.inner.updated_at()
    }

    pub fn insert_layer(
        &mut self,
        layer_name: String,
        layer: Layer,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<(), Error> {
        // Check if the layer already exists first
        if self.inner.layers.contains_key(&layer_name) {
            match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    // Will overwrite, so modify
                    self.inner.modify().layers.insert(layer_name, layer);
                    // Timestamp updated when guard drops
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring layer '{layer_name}': already exists in partition");
                    // No modification, no timestamp update
                }
                IfDuplicateBehavior::Error => {
                    return Err(Error::LayerAlreadyExists(layer_name));
                }
            }
        } else {
            self.inner.modify().layers.insert(layer_name, layer);
        }

        Ok(())
    }

    pub fn num_chunks(&self) -> u64 {
        self.inner
            .layers
            .values()
            .map(|layer| layer.num_chunks())
            .sum()
    }

    pub fn size_bytes(&self) -> u64 {
        self.inner
            .layers
            .values()
            .map(|layer| layer.size_bytes())
            .sum()
    }
}
