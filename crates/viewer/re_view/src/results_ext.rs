use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools as _;
use re_chunk_store::{Chunk, LatestAtQuery, RangeQuery};
use re_log_types::hash::Hash64;
use re_query::{LatestAtResults, RangeResults};
use re_sdk_types::ComponentIdentifier;
use re_viewer_context::{DataResult, ViewContext, typed_fallback_for};

use crate::chunks_with_component::ChunksWithComponent;

// ---

/// Wrapper that contains the results of a latest-at query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
pub struct HybridLatestAtResults<'a> {
    pub overrides: LatestAtResults,
    pub results: LatestAtResults,
    pub defaults: &'a LatestAtResults,

    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,

    /// Hash of mappings applied to [`Self::results`].
    pub component_mappings_hash: Hash64,
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridRangeResults<'a> {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: RangeResults,
    pub(crate) defaults: &'a LatestAtResults,

    /// Hash of mappings applied to [`Self::results`].
    pub(crate) component_mappings_hash: Hash64,
}

impl HybridLatestAtResults<'_> {
    /// Utility for retrieving the first instance of a component, ignoring defaults.
    #[inline]
    pub fn get_required_mono<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<C> {
        self.get_required_instance(0, component)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<C> {
        self.get_instance(0, component)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono_with_fallback<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> C {
        self.get_instance::<C>(0, component).unwrap_or_else(|| {
            let query_context = self.ctx.query_context(self.data_result, &self.query);
            typed_fallback_for(&query_context, component)
        })
    }

    /// Utility for retrieving a single instance of a component, not checking for defaults.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_required_instance<C: re_types_core::Component>(
        &self,
        index: usize,
        component: ComponentIdentifier,
    ) -> Option<C> {
        self.overrides
            .component_instance::<C>(index, component)
            .or_else(||
                // No override -> try recording store instead
                self.results.component_instance::<C>(index, component))
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance<C: re_types_core::Component>(
        &self,
        index: usize,
        component: ComponentIdentifier,
    ) -> Option<C> {
        self.get_required_instance(index, component).or_else(|| {
            // No override & no store -> try default instead
            self.defaults.component_instance::<C>(index, component)
        })
    }
}

pub enum HybridResults<'a> {
    LatestAt(LatestAtQuery, HybridLatestAtResults<'a>),
    Range(RangeQuery, HybridRangeResults<'a>),
}

impl HybridResults<'_> {
    pub fn query_result_hash(&self) -> Hash64 {
        // This is called very frequently, don't put a profile scope here.
        // TODO(andreas): We should be able to do better than this and determine hashes for queries on the fly.

        match self {
            Self::LatestAt(_, r) => {
                let mut indices = Vec::with_capacity(
                    // Don't add defaults component count because that's defaults for the entire view.
                    r.overrides.components.len() + r.results.components.len(),
                );

                indices.extend(
                    r.defaults
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );
                indices.extend(
                    r.overrides
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );
                indices.extend(
                    r.results
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );

                Hash64::hash((&indices, r.component_mappings_hash))
            }

            Self::Range(_, r) => {
                let mut indices = Vec::with_capacity(
                    // Don't add defaults component count because that's defaults for the entire view.
                    r.overrides.components.len() + r.results.components.len(),
                );

                indices.extend(
                    r.defaults
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );
                indices.extend(
                    r.overrides
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );
                indices.extend(r.results.components.iter().flat_map(|(component, chunks)| {
                    chunks
                        .iter()
                        .flat_map(|chunk| chunk.component_row_ids(*component))
                }));

                Hash64::hash((&indices, r.component_mappings_hash))
            }
        }
    }
}

// ---

impl<'a> From<(LatestAtQuery, HybridLatestAtResults<'a>)> for HybridResults<'a> {
    #[inline]
    fn from((query, results): (LatestAtQuery, HybridLatestAtResults<'a>)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl<'a> From<(RangeQuery, HybridRangeResults<'a>)> for HybridResults<'a> {
    #[inline]
    fn from((query, results): (RangeQuery, HybridRangeResults<'a>)) -> Self {
        Self::Range(query, results)
    }
}

/// Extension traits to abstract query result handling for all spatial views.
///
/// Also turns all results into range results, so that views only have to worry about the ranged
/// case.
pub trait RangeResultsExt {
    /// Returns component data for the given component, ignores default data if the result
    /// distinguishes them.
    ///
    /// For results that are aware of the blueprint, only overrides & store results will
    /// be considered.
    /// Defaults have no effect.
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>>;

    /// Returns component data for the given component or an empty array.
    ///
    /// For results that are aware of the blueprint, overrides, store results, and defaults will be
    /// considered.
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]>;

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    fn iter_as(
        &self,
        timeline: TimelineName,
        component: ComponentIdentifier,
    ) -> HybridResultsChunkIter<'_> {
        let chunks = self.get_optional_chunks(component);
        HybridResultsChunkIter {
            chunks,
            timeline,
            component,
        }
    }
}

impl RangeResultsExt for LatestAtResults {
    #[inline]
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>> {
        self.get(component)
            .cloned()
            .map(|chunk| ChunksWithComponent {
                chunks: Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]),
                component,
            })
    }

    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]> {
        self.get(component).cloned().map_or_else(
            || Cow::Owned(vec![]),
            |chunk| Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]),
        )
    }
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>> {
        self.get_required(component)
            .ok()
            .map(|chunks| ChunksWithComponent {
                chunks: Cow::Borrowed(chunks),
                component,
            })
    }

    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]> {
        Cow::Borrowed(self.get(component).unwrap_or_default())
    }
}

impl RangeResultsExt for HybridRangeResults<'_> {
    #[inline]
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>> {
        if let Some(unit) = self.overrides.get(component) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(ChunksWithComponent {
                chunks: Cow::Owned(vec![chunk]),
                component,
            })
        } else {
            self.results.get_required_chunks(component)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]> {
        re_tracing::profile_function!();

        if let Some(unit) = self.overrides.get(component) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            re_tracing::profile_scope!("defaults");

            // NOTE: Because this is a range query, we always need the defaults to come first,
            // since range queries don't have any state to bootstrap from.
            let defaults = self.defaults.get(component).map(|unit| {
                // Because this is an default from the blueprint we always re-index the data as static
                Arc::unwrap_or_clone(unit.clone().into_chunk())
                    .into_static()
                    .zeroed()
            });

            let chunks = self.results.get_optional_chunks(component);

            // TODO(cmc): this `collect_vec()` sucks, let's keep an eye on it and see if it ever
            // becomes an issue.
            Cow::Owned(
                defaults
                    .into_iter()
                    .chain(chunks.iter().cloned())
                    .collect_vec(),
            )
        }
    }
}

impl RangeResultsExt for HybridLatestAtResults<'_> {
    #[inline]
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>> {
        if let Some(unit) = self.overrides.get(component) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(ChunksWithComponent {
                chunks: Cow::Owned(vec![chunk]),
                component,
            })
        } else {
            self.results.get_required_chunks(component)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]> {
        if let Some(unit) = self.overrides.get(component) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            let chunks = self
                .results
                .get_optional_chunks(component)
                .iter()
                // NOTE: Since this is a latest-at query that is being coerced into a range query, we
                // need to make sure that every secondary column has an index smaller then the primary column
                // (we use `(TimeInt::STATIC, RowId::ZERO)`), otherwise range zipping would yield unexpected
                // results.
                .map(|chunk| chunk.clone().into_static().zeroed())
                .collect_vec();

            // If the data is not empty, return it.
            if !chunks.is_empty() {
                return Cow::Owned(chunks);
            }

            // Otherwise try to use the default data.
            let Some(unit) = self.defaults.get(component) else {
                return Cow::Owned(Vec::new());
            };
            // Because this is an default from the blueprint we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        }
    }
}

impl RangeResultsExt for HybridResults<'_> {
    #[inline]
    fn get_required_chunks(
        &self,
        component: ComponentIdentifier,
    ) -> Option<ChunksWithComponent<'_>> {
        match self {
            Self::LatestAt(_, results) => results.get_required_chunks(component),
            Self::Range(_, results) => results.get_required_chunks(component),
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> Cow<'_, [Chunk]> {
        match self {
            Self::LatestAt(_, results) => results.get_optional_chunks(component),
            Self::Range(_, results) => results.get_optional_chunks(component),
        }
    }
}

// ---

use re_chunk::{ChunkComponentIterItem, RowId, TimeInt, TimelineName};
use re_chunk_store::external::re_chunk;

/// The iterator type backing [`HybridResults::iter_as`].
pub struct HybridResultsChunkIter<'a> {
    chunks: Cow<'a, [Chunk]>,
    timeline: TimelineName,
    component: ComponentIdentifier,
}

impl<'a> HybridResultsChunkIter<'a> {
    /// Iterate as indexed deserialized batches.
    ///
    /// TODO(#5305): Note that this uses the old codegen'd deserialization path, which does some
    /// very unidiomatic Arrow things, and is therefore very slow at the moment. Avoid this on
    /// performance critical paths.
    ///
    /// See [`Chunk::iter_component`] for more information.
    pub fn component_slow<C: re_types_core::Component>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), ChunkComponentIterItem<C>)> + 'a {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(self.timeline, self.component),
                chunk.iter_component::<C>(self.component),
            )
        })
    }

    /// Iterate as indexed, sliced, deserialized component batches.
    ///
    /// See [`Chunk::iter_slices`] for more information.
    pub fn slice<S: 'a + re_chunk::ChunkComponentSlicer>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), S::Item<'a>)> + 'a {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(self.timeline, self.component),
                chunk.iter_slices::<S>(self.component),
            )
        })
    }

    /// Iterate as indexed, sliced, deserialized component batches for a specific struct field.
    ///
    /// See [`Chunk::iter_slices_from_struct_field`] for more information.
    pub fn slice_from_struct_field<S: 'a + re_chunk::ChunkComponentSlicer>(
        &'a self,
        field_name: &'a str,
    ) -> impl Iterator<Item = ((TimeInt, RowId), S::Item<'a>)> + 'a {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(self.timeline, self.component),
                chunk.iter_slices_from_struct_field::<S>(self.component, field_name)
            )
        })
    }
}
