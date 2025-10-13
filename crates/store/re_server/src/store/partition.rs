use std::collections::HashMap;

use re_chunk_store::ChunkStoreHandle;

use crate::store::Layer;

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

    pub fn layer(&self, layer_name: &str) -> Option<&Layer> {
        self.layers.get(layer_name)
    }

    pub fn last_updated_at(&self) -> jiff::Timestamp {
        self.last_updated_at
    }

    pub fn insert_layer(&mut self, layer_name: String, layer: Layer) {
        self.layers.insert(layer_name, layer);
        self.last_updated_at = jiff::Timestamp::now();
    }

    pub fn num_chunks(&self) -> u64 {
        self.layers.values().map(|layer| layer.num_chunks()).sum()
    }

    pub fn size_bytes(&self) -> u64 {
        self.layers.values().map(|layer| layer.size_bytes()).sum()
    }

    pub fn iter_store_handles(&self) -> impl Iterator<Item = &ChunkStoreHandle> {
        self.layers.values().map(|layer| layer.store_handle())
    }
}
