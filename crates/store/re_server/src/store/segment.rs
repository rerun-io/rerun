use std::collections::HashMap;

use itertools::Itertools as _;
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

/// What happened to a segment's layer map as a result of an
/// [`Segment::insert_layer`] call.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerInsertOutcome {
    /// The layer name was not previously present; the new layer was added.
    Inserted,

    /// The layer name was already present; the existing layer was replaced
    /// (per [`IfDuplicateBehavior::Overwrite`]).
    Overwritten,

    /// The layer name was already present and the existing layer was kept
    /// (per [`IfDuplicateBehavior::Skip`]). No mutation occurred.
    Skipped,
}

impl Segment {
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

    /// Insert a layer into this segment, observing `on_duplicate` if the
    /// layer name is already present.
    ///
    /// Returns:
    /// - `Ok(Inserted)`    on fresh insert
    /// - `Ok(Overwritten)` if the layer existed and `on_duplicate = Overwrite`
    /// - `Ok(Skipped)`     if the layer existed and `on_duplicate = Skip`
    ///   (no mutation occurs; the existing layer is unchanged)
    /// - `Err(LayerAlreadyExists)` if the layer existed and
    ///   `on_duplicate = Error`
    pub fn insert_layer(
        &mut self,
        layer_name: String,
        layer: Layer,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<LayerInsertOutcome, Error> {
        if self.inner.layers.contains_key(&layer_name) {
            match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    // Will overwrite, so modify
                    self.inner.modify().layers.insert(layer_name, layer);
                    // Timestamp updated when guard drops
                    Ok(LayerInsertOutcome::Overwritten)
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring layer '{layer_name}': already exists in segment");
                    // No modification, no timestamp update
                    Ok(LayerInsertOutcome::Skipped)
                }
                IfDuplicateBehavior::Error => Err(Error::LayerAlreadyExists(layer_name)),
            }
        } else {
            self.inner.modify().layers.insert(layer_name, layer);
            Ok(LayerInsertOutcome::Inserted)
        }
    }

    /// Returns the removed [`Layer`], if any.
    pub fn remove_layer(&mut self, layer_name: &str) -> Option<Layer> {
        self.inner.modify().layers.remove(layer_name)
    }

    /// Retains only the layers specified by the predicate.
    ///
    /// In other words, remove all pairs `(name, layer)` for which `f(&name, &mut layer)` returns `false`.
    /// The layers are visited in unsorted (and unspecified) order.
    pub fn retain_layers<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut Layer) -> bool,
    {
        self.inner.modify().layers.retain(f);
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
