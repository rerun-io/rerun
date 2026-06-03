use std::{collections::HashMap, sync::Arc};

use itertools::Itertools as _;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_types_core::LayerName;

use crate::store::{Error, Source, Tracked};

/// The mutable inner state of a [`Segment`], wrapped in [`Tracked`] for automatic timestamp updates.
#[derive(Clone)]
pub struct SegmentInner {
    /// The sources for all the layers this segment belongs to.
    sources: HashMap<LayerName, Arc<Source>>,
}

#[derive(Clone)]
pub struct Segment {
    inner: Tracked<SegmentInner>,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            inner: Tracked::new(SegmentInner {
                sources: HashMap::default(),
            }),
        }
    }
}

/// What happened to a segment's layer map as a result of an
/// [`Segment::insert_segment`] call.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceInsertOutcome {
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
    pub fn source_count(&self) -> usize {
        self.inner.sources.len()
    }

    pub fn sources(&self) -> &HashMap<LayerName, Arc<Source>> {
        &self.inner.sources
    }

    /// Iterate over the layers in this segments.
    ///
    /// Layers are iterated in (registration time, layer name) order,
    /// as per how they should appear in the segment table.
    pub fn iter_sources(&self) -> impl Iterator<Item = (&LayerName, &Source)> {
        self.inner
            .sources
            .iter()
            .sorted_by(|(name_a, source_a), (name_b, source_b)| {
                (source_a.registration_time(), name_a).cmp(&(source_b.registration_time(), name_b))
            })
            .map(|(name, source)| (name, source.as_ref()))
    }

    pub fn source(&self, layer_name: &LayerName) -> Option<&Source> {
        self.inner.sources.get(layer_name).map(|s| s.as_ref())
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
    pub fn insert_source(
        &mut self,
        source: Arc<Source>,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<SourceInsertOutcome, Error> {
        let layer_name = source.layer_info().name.clone();
        if self.inner.sources.contains_key(&layer_name) {
            match on_duplicate {
                IfDuplicateBehavior::Overwrite => {
                    // Will overwrite, so modify
                    self.inner.modify().sources.insert(layer_name, source);
                    // Timestamp updated when guard drops
                    Ok(SourceInsertOutcome::Overwritten)
                }
                IfDuplicateBehavior::Skip => {
                    re_log::info!("Ignoring layer '{layer_name}': already exists in segment");
                    // No modification, no timestamp update
                    Ok(SourceInsertOutcome::Skipped)
                }
                IfDuplicateBehavior::Error => Err(Error::LayerAlreadyExists(layer_name)),
            }
        } else {
            self.inner.modify().sources.insert(layer_name, source);
            Ok(SourceInsertOutcome::Inserted)
        }
    }

    /// Returns the removed [`Source`], if any.
    pub fn remove_source(&mut self, layer_name: &LayerName) -> Option<Arc<Source>> {
        self.inner.modify().sources.remove(layer_name)
    }

    /// Retains only the sources specified by the predicate.
    ///
    /// In other words, remove all pairs `(name, source)` for which `f(&name, &mut source)` returns `false`.
    /// The sources are visited in unsorted (and unspecified) order.
    pub fn retain_sources<F>(&mut self, mut f: F)
    where
        F: FnMut(&LayerName, &Source) -> bool,
    {
        self.inner
            .modify()
            .sources
            .retain(|name, source| f(name, source.as_ref()));
    }

    pub fn num_chunks(&self) -> u64 {
        self.inner
            .sources
            .values()
            .map(|source| source.num_chunks())
            .sum()
    }

    pub fn size_bytes(&self) -> u64 {
        self.inner
            .sources
            .values()
            .map(|source| source.size_bytes())
            .sum()
    }
}
