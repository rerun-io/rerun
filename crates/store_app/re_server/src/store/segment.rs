use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Fields, Schema};
use itertools::Itertools as _;
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_types_core::LayerName;

use crate::store::{Error, Source, Tracked};

/// The mutable inner state of a [`Segment`], wrapped in [`Tracked`] for automatic timestamp updates.
#[derive(Clone, Default)]
pub struct SegmentInner {
    /// The sources for all the layers this segment belongs to.
    sources: HashMap<LayerName, Arc<Source>>,
}

#[derive(Clone, Default)]
pub struct Segment {
    inner: Tracked<SegmentInner>,
}

/// What happened to a segment's layer map as a result of an
/// [`Segment::insert_source`] call.
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

    /// Compute this segment's merged properties as a one-row [`RecordBatch`].
    ///
    /// Properties are accumulated across the segment's layers in registration order, so the
    /// last registered layer wins on conflicts.
    pub async fn compute_properties(&self) -> Result<RecordBatch, Error> {
        let mut properties = BTreeMap::default();

        for (_layer_name, layer) in self.iter_sources() {
            let layer_properties = layer.compute_properties().await?;
            for (col_idx, field) in layer_properties.schema().fields().iter().enumerate() {
                properties.insert(
                    Arc::clone(field),
                    Arc::clone(layer_properties.column(col_idx)),
                );
            }
        }

        RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(
                properties.keys().map(Arc::clone).collect::<Fields>(),
                Default::default(),
            )),
            properties.into_values().collect(),
            // Exactly one row per segment. We must state it explicitly so Arrow can infer the
            // row count even when the segment has no properties at all.
            &RecordBatchOptions::default().with_row_count(Some(1)),
        )
        .map_err(Error::failed_to_extract_properties)
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
