use std::{borrow::Cow, sync::Arc};

use itertools::Itertools as _;

use re_chunk_store::{Chunk, LatestAtQuery, RangeQuery};
use re_log_types::hash::Hash64;
use re_query::{LatestAtResults, RangeResults};
use re_types::ComponentDescriptor;
use re_types_core::ComponentName;
use re_viewer_context::{DataResult, QueryContext, ViewContext};

use crate::{DataResultQuery as _, chunks_with_descriptor::ChunksWithDescriptor};

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
}

impl HybridLatestAtResults<'_> {
    // TODO(#10112): Right now, fallbacks are on a per-component basis, so it's fine to pass the component name here.
    pub fn fallback_raw(&self, component_name: ComponentName) -> arrow::array::ArrayRef {
        let query_context = QueryContext {
            view_ctx: self.ctx,
            target_entity_path: &self.data_result.entity_path,
            archetype_name: None, // TODO(jleibs): Do we need this?
            query: &self.query,
        };

        // TODO(#10112): This is quite expensive for what we're doing here, but it would be trivial to remove with the proposal in #10112.
        let visualizer_collection = self.ctx.new_visualizer_collection();
        self.data_result
            .best_fallback_for(&query_context, &visualizer_collection, component_name)
    }

    /// Utility for retrieving the first instance of a component, ignoring defaults.
    #[inline]
    pub fn get_required_mono<C: re_types_core::Component>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Option<C> {
        self.get_required_instance(0, component_descr)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono<C: re_types_core::Component>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> Option<C> {
        self.get_instance(0, component_descr)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono_with_fallback<C: re_types_core::Component + Default>(
        &self,
        component_descr: &ComponentDescriptor,
    ) -> C {
        debug_assert_eq!(component_descr.component_name, C::name());

        self.get_instance_with_fallback(0, component_descr)
    }

    /// Utility for retrieving a single instance of a component, not checking for defaults.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_required_instance<C: re_types_core::Component>(
        &self,
        index: usize,
        component_descr: &ComponentDescriptor,
    ) -> Option<C> {
        self.overrides
            .component_instance::<C>(index, component_descr)
            .or_else(||
                // No override -> try recording store instead
                self.results.component_instance::<C>(index, component_descr))
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance<C: re_types_core::Component>(
        &self,
        index: usize,
        component_descr: &ComponentDescriptor,
    ) -> Option<C> {
        debug_assert_eq!(component_descr.component_name, C::name());

        self.get_required_instance(index, component_descr)
            .or_else(|| {
                // No override & no store -> try default instead
                self.defaults
                    .component_instance::<C>(index, component_descr)
            })
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance_with_fallback<C: re_types_core::Component + Default>(
        &self,
        index: usize,
        component_descr: &ComponentDescriptor,
    ) -> C {
        debug_assert_eq!(component_descr.component_name, C::name());

        let component_name = component_descr.component_name;
        self.get_instance(index, component_descr)
            .or_else(|| {
                // No override, no store, no default -> try fallback instead
                let raw_fallback = self.fallback_raw(component_name);
                C::from_arrow(raw_fallback.as_ref())
                    .ok()
                    .and_then(|r| r.first().cloned())
            })
            .unwrap_or_default()
    }
}

pub enum HybridResults<'a> {
    LatestAt(LatestAtQuery, HybridLatestAtResults<'a>),

    // Boxed because of size difference between variants
    Range(RangeQuery, Box<HybridRangeResults<'a>>),
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

                Hash64::hash(&indices)
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
                indices.extend(r.results.components.iter().flat_map(
                    |(component_descriptor, chunks)| {
                        chunks
                            .iter()
                            .flat_map(|chunk| chunk.component_row_ids(component_descriptor))
                    },
                ));

                Hash64::hash(&indices)
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
        Self::Range(query, Box::new(results))
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
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>>;

    /// Returns component data for the given component or an empty array.
    ///
    /// For results that are aware of the blueprint, overrides, store results, and defaults will be
    /// considered.
    // TODO(#6889): Take descriptor instead of name.
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]>;

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    fn iter_as(
        &self,
        timeline: TimelineName,
        component_descriptor: ComponentDescriptor,
    ) -> HybridResultsChunkIter<'_> {
        let chunks = self.get_optional_chunks(component_descriptor.clone());
        HybridResultsChunkIter {
            chunks,
            timeline,
            component_descriptor,
        }
    }
}

impl RangeResultsExt for LatestAtResults {
    #[inline]
    fn get_required_chunks(
        &self,
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>> {
        self.get(&component_descriptor)
            .cloned()
            .map(|chunk| ChunksWithDescriptor {
                chunks: Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]),
                component_descriptor,
            })
    }

    #[inline]
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]> {
        self.get(&component_descriptor).cloned().map_or_else(
            || Cow::Owned(vec![]),
            |chunk| Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]),
        )
    }
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_required_chunks(
        &self,
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>> {
        self.get_required(&component_descriptor)
            .ok()
            .map(|chunks| ChunksWithDescriptor {
                chunks: Cow::Borrowed(chunks),
                component_descriptor,
            })
    }

    #[inline]
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]> {
        Cow::Borrowed(self.get(&component_descriptor).unwrap_or_default())
    }
}

impl RangeResultsExt for HybridRangeResults<'_> {
    // TODO(andreas): We typically lookup inside the returned chunks using the same descriptor.
    // We should return a more highlevel type that doesn't require passing the descriptor again!
    #[inline]
    fn get_required_chunks(
        &self,
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>> {
        if let Some(unit) = self.overrides.get(&component_descriptor) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(ChunksWithDescriptor {
                chunks: Cow::Owned(vec![chunk]),
                component_descriptor,
            })
        } else {
            self.results.get_required_chunks(component_descriptor)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]> {
        re_tracing::profile_function!();

        if let Some(unit) = self.overrides.get(&component_descriptor) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            re_tracing::profile_scope!("defaults");

            // NOTE: Because this is a range query, we always need the defaults to come first,
            // since range queries don't have any state to bootstrap from.
            let defaults = self.defaults.get(&component_descriptor).map(|unit| {
                // Because this is an default from the blueprint we always re-index the data as static
                Arc::unwrap_or_clone(unit.clone().into_chunk())
                    .into_static()
                    .zeroed()
            });

            let chunks = self.results.get_optional_chunks(component_descriptor);

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
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>> {
        if let Some(unit) = self.overrides.get(&component_descriptor) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(ChunksWithDescriptor {
                chunks: Cow::Owned(vec![chunk]),
                component_descriptor,
            })
        } else {
            self.results.get_required_chunks(component_descriptor)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]> {
        if let Some(unit) = self.overrides.get(&component_descriptor) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            let chunks = self
                .results
                .get_optional_chunks(component_descriptor.clone())
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
            let Some(unit) = self.defaults.get(&component_descriptor) else {
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
        component_descriptor: ComponentDescriptor,
    ) -> Option<ChunksWithDescriptor<'_>> {
        match self {
            Self::LatestAt(_, results) => results.get_required_chunks(component_descriptor),
            Self::Range(_, results) => results.get_required_chunks(component_descriptor),
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_descriptor: ComponentDescriptor) -> Cow<'_, [Chunk]> {
        match self {
            Self::LatestAt(_, results) => results.get_optional_chunks(component_descriptor),
            Self::Range(_, results) => results.get_optional_chunks(component_descriptor),
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
    component_descriptor: ComponentDescriptor,
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
                chunk.iter_component_indices(&self.timeline, &self.component_descriptor),
                chunk.iter_component::<C>(&self.component_descriptor),
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
                chunk.iter_component_indices(&self.timeline, &self.component_descriptor),
                chunk.iter_slices::<S>(self.component_descriptor.clone()),
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
                chunk.iter_component_indices(&self.timeline, &self.component_descriptor),
                chunk.iter_slices_from_struct_field::<S>(
                    self.component_descriptor.clone(),
                    field_name,
                )
            )
        })
    }
}
