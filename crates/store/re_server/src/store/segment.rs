use std::collections::HashMap;

use itertools::Itertools as _;
use re_chunk_store::ChunkStoreHandle;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;

use crate::store::{Error, Layer, Tracked};

/// The mutable inner state of a [`Segment`], wrapped in [`Tracked`] for automatic timestamp updates.
#[derive(Clone)]
pub struct SegmentInner {
    /// The layers of this segment.
    layers: HashMap<String, Layer>,
}

#[derive(Clone)]
pub struct Segment {
    inner: Tracked<SegmentInner>,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            inner: Tracked::new(SegmentInner {
                layers: HashMap::default(),
            }),
        }
    }
}

impl Segment {
    pub fn from_layer_data(layer_name: &str, chunk_store_handle: ChunkStoreHandle) -> Self {
        Self {
            inner: Tracked::new(SegmentInner {
                layers: vec![(layer_name.to_owned(), Layer::new(chunk_store_handle))]
                    .into_iter()
                    .collect(),
            }),
        }
    }

    pub fn layer_count(&self) -> usize {
        self.inner.layers.len()
    }

    pub fn layers(&self) -> &HashMap<String, Layer> {
        &self.inner.layers
    }

    /// Iterate over layers.
    ///
    /// Layers are iterated in (registration time, layer name) order, as per how they should appear
    /// in the segment table.
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

    /// Result: a successful result is `true` if the layer existed and was overwritten
    pub fn insert_layer(
        &mut self,
        layer_name: String,
        layer: Layer,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<bool, Error> {
        // Check if the layer already exists first
        if self.inner.layers.contains_key(&layer_name) {
            match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    // Will overwrite, so modify
                    self.inner.modify().layers.insert(layer_name, layer);
                    // Timestamp updated when guard drops
                    Ok(true)
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring layer '{layer_name}': already exists in segment");
                    // No modification, no timestamp update
                    Ok(true)
                }
                IfDuplicateBehavior::Error => Err(Error::LayerAlreadyExists(layer_name)),
            }
        } else {
            self.inner.modify().layers.insert(layer_name, layer);
            Ok(false)
        }
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
