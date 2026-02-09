use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk_store::{LatestAtQuery, RangeQuery};
use re_log_types::hash::Hash64;
use re_query::{LatestAtResults, RangeResults};
use re_sdk_types::blueprint::datatypes::ComponentSourceKind;
use re_sdk_types::{ComponentIdentifier, blueprint::components::VisualizerInstructionId};
use re_viewer_context::{DataResult, ViewContext, typed_fallback_for};

use crate::{
    ComponentMappingError,
    chunks_with_component::{ChunksWithComponent, MaybeChunksWithComponent},
};

// ---

/// Wrapper that contains the results of a latest-at query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::BlueprintResolvedResultsExt`] trait
/// they will be merged into the results appropriately.
pub struct BlueprintResolvedLatestAtResults<'a> {
    pub overrides: LatestAtResults,
    pub store_results: LatestAtResults,
    pub view_defaults: &'a LatestAtResults,
    pub(crate) instruction_id: Option<VisualizerInstructionId>,

    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,

    pub component_sources:
        IntMap<ComponentIdentifier, Result<ComponentSourceKind, ComponentMappingError>>,

    /// Hash of mappings applied to [`Self::store_results`].
    pub component_indices_hash: Hash64,
}

impl BlueprintResolvedLatestAtResults<'_> {
    /// Are there any chunks that need to be fetched from a remote store?
    pub fn any_missing_chunks(&self) -> bool {
        0 < self.overrides.missing_virtual.len()
            + self.store_results.missing_virtual.len()
            + self.view_defaults.missing_virtual.len()
    }
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::BlueprintResolvedResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct BlueprintResolvedRangeResults<'a> {
    pub(crate) overrides: LatestAtResults,
    pub(crate) store_results: RangeResults,
    pub(crate) view_defaults: &'a LatestAtResults,
    pub(crate) _instruction_id: VisualizerInstructionId,

    pub(crate) component_sources:
        IntMap<ComponentIdentifier, Result<ComponentSourceKind, ComponentMappingError>>,

    /// Hash of mappings applied to [`Self::store_results`].
    pub(crate) component_mappings_hash: Hash64,
}

impl BlueprintResolvedRangeResults<'_> {
    /// Are there any chunks that need to be fetched from a remote store?
    fn any_missing_chunks(&self) -> bool {
        0 < self.overrides.missing_virtual.len()
            + self.store_results.missing_virtual.len()
            + self.view_defaults.missing_virtual.len()
    }

    /// Merges bootstrapped data from a latest-at query into this range query result.
    ///
    /// Latest-at bootstrapping is used for optional/recommended components (like colors, radii,
    /// labels, etc.) that should maintain stable values at the beginning of a visible time range.
    /// Without bootstrapping, these components would only appear where they have data within the
    /// visible range, causing them to change unexpectedly as the time window moves.
    ///
    /// For example, if a plot's color was set at t=50 and you're viewing t=100-200, you want
    /// that color to persist throughout the range rather than disappearing or changing.
    ///
    /// Bootstrapped results are prepended to the store results and converted to static,
    /// zeroed chunks to allow proper range zipping. Any errors from the bootstrap query
    /// are also merged into the component sources.
    // TODO(andreas): It's a bit overkill to do a full blueprint resolved query for both the range & latest-at part. This can be optimized!
    pub fn merge_bootstrapped_data(&mut self, bootstrapped: BlueprintResolvedLatestAtResults<'_>) {
        // Copy component sources from bootstrap if they indicate errors.
        #[expect(clippy::iter_over_hash_type)] // Fills up another hash type.
        for (component, source_result) in bootstrapped.component_sources {
            if let Err(err) = source_result {
                // If bootstrapping failed for a component, record the error (potentially overwriting an existing error)
                self.component_sources.insert(component, Err(err));
            }
        }

        // Prepend bootstrapped chunks to range results
        #[expect(clippy::iter_over_hash_type)] // Fills up another hash type.
        for (component, unit_chunk) in bootstrapped.store_results.components {
            // Convert to a static, zeroed chunk for proper range zipping
            let chunk = Arc::unwrap_or_clone(unit_chunk.into_chunk())
                .into_static()
                .zeroed();

            // Prepend bootstrapped chunk to range results
            self.store_results
                .components
                .entry(component)
                .or_default()
                .insert(0, chunk);
        }
    }
}

impl BlueprintResolvedLatestAtResults<'_> {
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
            let query_context = if let Some(instruction_id) = self.instruction_id {
                self.ctx
                    .query_context(self.data_result, &self.query, instruction_id)
            } else {
                self.ctx
                    .query_context_without_visualizer(self.data_result, &self.query)
            };
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
                self.store_results.component_instance::<C>(index, component))
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
            self.view_defaults.component_instance::<C>(index, component)
        })
    }
}

/// An enum wrapping either latest-at or range blueprint resolved results.
pub enum BlueprintResolvedResults<'a> {
    LatestAt(LatestAtQuery, BlueprintResolvedLatestAtResults<'a>),
    Range(RangeQuery, BlueprintResolvedRangeResults<'a>),
}

impl BlueprintResolvedResults<'_> {
    pub fn timeline(&self) -> re_log_types::TimelineName {
        match self {
            Self::LatestAt(query, _) => query.timeline(),
            Self::Range(query, _) => *query.timeline(),
        }
    }

    /// Are there any chunks that need to be fetched from a remote store?
    pub fn any_missing_chunks(&self) -> bool {
        match self {
            Self::LatestAt(_, results) => results.any_missing_chunks(),
            Self::Range(_, results) => results.any_missing_chunks(),
        }
    }

    pub fn query_result_hash(&self) -> Hash64 {
        // This is called very frequently, don't put a profile scope here.
        // TODO(andreas): We should be able to do better than this and determine hashes for queries on the fly.

        match self {
            Self::LatestAt(_, r) => {
                let mut indices = Vec::with_capacity(
                    // Don't add defaults component count because that's defaults for the entire view.
                    r.overrides.components.len() + r.store_results.components.len(),
                );

                indices.extend(
                    r.view_defaults
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
                    r.store_results
                        .components
                        .values()
                        .filter_map(|chunk| chunk.row_id()),
                );

                Hash64::hash((&indices, r.component_indices_hash))
            }

            Self::Range(_, r) => {
                let mut indices = Vec::with_capacity(
                    // Don't add defaults component count because that's defaults for the entire view.
                    r.overrides.components.len() + r.store_results.components.len(),
                );

                indices.extend(
                    r.view_defaults
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
                indices.extend(r.store_results.components.iter().flat_map(
                    |(component, chunks)| {
                        chunks
                            .iter()
                            .flat_map(|chunk| chunk.component_row_ids(*component))
                    },
                ));

                Hash64::hash((&indices, r.component_mappings_hash))
            }
        }
    }
}

// ---

impl<'a> From<(LatestAtQuery, BlueprintResolvedLatestAtResults<'a>)>
    for BlueprintResolvedResults<'a>
{
    #[inline]
    fn from((query, results): (LatestAtQuery, BlueprintResolvedLatestAtResults<'a>)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl<'a> From<(RangeQuery, BlueprintResolvedRangeResults<'a>)> for BlueprintResolvedResults<'a> {
    #[inline]
    fn from((query, results): (RangeQuery, BlueprintResolvedRangeResults<'a>)) -> Self {
        Self::Range(query, results)
    }
}

/// Extension traits to abstract query result handling for all spatial views.
///
/// Also turns all results into range results, so that views only have to worry about the ranged
/// case.
pub trait BlueprintResolvedResultsExt<'a> {
    /// Returns component data for the given component or an empty array.
    ///
    /// For results that are aware of the blueprint, overrides, store results, and defaults will be
    /// considered.
    ///
    /// `force_preserve_store_row_ids`: If true, preserves row IDs from store data in latest-at queries.
    /// If false, all results are re-indexed to ([`TimeInt::STATIC`], [`RowId::ZERO`])
    /// in order to allow the same range zipping.
    /// When false, you cannot rely on row ids for any hashing/identification purposes!
    ///
    /// **WARNING:** Blueprint data (overrides/defaults) is **always** re-indexed to
    /// ([`TimeInt::STATIC`], [`RowId::ZERO`]) regardless of this setting.
    fn get_chunks(
        &'a self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> MaybeChunksWithComponent<'a>;

    /// Returns required component chunks with preserved store row IDs.
    ///
    /// Use this for required components where row IDs are needed for caching or identification.
    ///
    /// Blueprint row IDs are always discarded.
    #[inline]
    fn get_required_chunks(
        &'a self,
        component: ComponentIdentifier,
    ) -> MaybeChunksWithComponent<'a> {
        self.get_chunks(component, true)
    }

    /// Returns optional component chunks with zeroed store row IDs.
    ///
    /// Use this for optional/recommended components where the original row IDs would otherwise
    /// interfere with range zipping on latest-at queries.
    ///
    /// Blueprint row IDs are always discarded.
    #[inline]
    fn get_optional_chunks(
        &'a self,
        component: ComponentIdentifier,
    ) -> MaybeChunksWithComponent<'a> {
        self.get_chunks(component, false)
    }

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Reports an error if there's no chunks for the given component and returns an empty iterator.
    /// Use this for required components where row IDs are needed for caching or identification.
    ///
    /// Blueprint row IDs are always discarded.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    fn iter_required(
        &'a self,
        mut reporter: impl FnMut(&ComponentMappingError),
        timeline: TimelineName,
        component: ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        let chunks_with_component = match self.get_required_chunks(component).try_into() {
            Ok(chunks) => chunks,
            Err(err) => {
                reporter(&err);
                ChunksWithComponent::empty(component)
            }
        };

        HybridResultsChunkIter {
            chunks_with_component,
            timeline,
        }
    }

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Does *not* report an error if there's no chunks for the given component and returns an empty iterator.
    /// Use this for optional/recommended components where the original row IDs would otherwise
    /// interfere with range zipping on latest-at queries.
    ///
    /// **WARNING**: For latest-at queries, the row IDs are always zeroed out to allow for range zipping.
    /// Blueprint row IDs are always discarded.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    fn iter_optional(
        &'a self,
        mut reporter: impl FnMut(&ComponentMappingError),
        timeline: TimelineName,
        component: ComponentIdentifier,
    ) -> HybridResultsChunkIter<'a> {
        let chunks_with_component = match self.get_optional_chunks(component).try_into() {
            Ok(chunks) => chunks,
            Err(err) => {
                reporter(&err);
                ChunksWithComponent::empty(component)
            }
        };

        HybridResultsChunkIter {
            chunks_with_component,
            timeline,
        }
    }
}

impl BlueprintResolvedResultsExt<'_> for BlueprintResolvedRangeResults<'_> {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        _force_preserve_store_row_ids: bool,
    ) -> MaybeChunksWithComponent<'_> {
        let source = match self.component_sources.get(&component) {
            Some(Ok(source)) => source,
            // TODO(grtlr,andreas): Not all of our errors implement clone (looking at you `ArrowError`)!
            Some(Err(err)) => return MaybeChunksWithComponent::error(component, err.clone()),
            None => return MaybeChunksWithComponent::empty(component),
        };

        let chunks = match source {
            ComponentSourceKind::SourceComponent => {
                // NOTE: Because this is a range query, we always need the defaults to come first,
                // since range queries don't have any state to bootstrap from.
                let defaults = self.view_defaults.get(component).map(|unit| {
                    // Because this is a default (blueprint data) we always re-index the data as static
                    // and zero the row IDs
                    Arc::unwrap_or_clone(unit.clone().into_chunk())
                        .into_static()
                        .zeroed()
                });

                let results_chunks = self.store_results.get(component).unwrap_or_default();

                // TODO(cmc): this `collect_vec()` sucks, let's keep an eye on it and see if it ever
                // becomes an issue.
                Cow::Owned(
                    defaults
                        .into_iter()
                        .chain(results_chunks.iter().cloned())
                        .collect_vec(),
                )
            }
            ComponentSourceKind::Override => {
                self.overrides
                    .get(component)
                    .map_or(Cow::Owned(Vec::new()), |unit| {
                        // Because this is an override (blueprint data) we always re-index the data as static
                        // and zero the row IDs
                        let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                            .into_static()
                            .zeroed();
                        Cow::Owned(vec![chunk])
                    })
            }
            ComponentSourceKind::Default => {
                self.view_defaults
                    .get(component)
                    .map_or(Cow::Owned(Vec::new()), |unit| {
                        // Because this is a default (blueprint data) we always re-index the data as static
                        // and zero the row IDs
                        Cow::Owned(vec![
                            Arc::unwrap_or_clone(unit.clone().into_chunk())
                                .into_static()
                                .zeroed(),
                        ])
                    })
            }
        };

        MaybeChunksWithComponent {
            maybe_chunks: Ok(chunks),
            component,
        }
    }
}

impl<'a> BlueprintResolvedResultsExt<'a> for BlueprintResolvedLatestAtResults<'_> {
    #[inline]
    fn get_chunks(
        &'a self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> MaybeChunksWithComponent<'a> {
        let source = match self.component_sources.get(&component) {
            Some(Ok(source)) => source,
            // TODO(grtlr,andreas): Not all of our errors implement clone (looking at you `ArrowError`)!
            Some(Err(err)) => return MaybeChunksWithComponent::error(component, err.clone()),
            None => return MaybeChunksWithComponent::empty(component),
        };

        let unit_chunk = match source {
            ComponentSourceKind::SourceComponent => {
                let chunks = self.store_results.get(component).cloned().map_or_else(
                    || Cow::Owned(vec![]),
                    |chunk| {
                        let mut chunk = Arc::unwrap_or_clone(chunk.into_chunk());
                        if !force_preserve_store_row_ids {
                            chunk = chunk.into_static().zeroed();
                        }
                        Cow::Owned(vec![chunk])
                    },
                );
                return MaybeChunksWithComponent {
                    maybe_chunks: Ok(chunks),
                    component,
                };
            }
            ComponentSourceKind::Override => self.overrides.get(component),
            ComponentSourceKind::Default => self.view_defaults.get(component),
        };

        if let Some(unit_chunk) = unit_chunk {
            // Because this is an override or default from the blueprint we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit_chunk.clone().into_chunk())
                .into_static()
                .zeroed();

            MaybeChunksWithComponent {
                maybe_chunks: Ok(Cow::Owned(vec![chunk])),

                component,
            }
        } else {
            MaybeChunksWithComponent::empty(component)
        }
    }
}

impl<'a> BlueprintResolvedResultsExt<'a> for BlueprintResolvedResults<'_> {
    #[inline]
    fn get_chunks(
        &'a self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> MaybeChunksWithComponent<'a> {
        match self {
            Self::LatestAt(_, results) => {
                results.get_chunks(component, force_preserve_store_row_ids)
            }
            Self::Range(_, results) => results.get_chunks(component, force_preserve_store_row_ids),
        }
    }
}

// ---

use re_chunk::{ChunkComponentIterItem, RowId, TimeInt, TimelineName};
use re_chunk_store::external::re_chunk;

/// The iterator type backing [`BlueprintResolvedResultsExt::iter_required`] and [`BlueprintResolvedResultsExt::iter_optional`].
pub struct HybridResultsChunkIter<'a> {
    chunks_with_component: ChunksWithComponent<'a>,
    timeline: TimelineName,
}

impl<'a> HybridResultsChunkIter<'a> {
    pub fn new(chunks_with_component: ChunksWithComponent<'a>, timeline: TimelineName) -> Self {
        Self {
            chunks_with_component,
            timeline,
        }
    }

    /// True if there's no chunks to iterate on.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chunks_with_component.chunks.is_empty()
    }

    /// Iterate as indexed deserialized batches.
    ///
    /// TODO(#5305): Note that this uses the old codegen'd deserialization path, which does some
    /// very unidiomatic Arrow things, and is therefore very slow at the moment. Avoid this on
    /// performance critical paths.
    ///
    /// See [`re_chunk::Chunk::iter_component`] for more information.
    pub fn component_slow<C: re_types_core::Component>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), ChunkComponentIterItem<C>)> + 'a {
        self.chunks_with_component
            .chunks
            .iter()
            .flat_map(move |chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(
                        self.timeline,
                        self.chunks_with_component.component
                    ),
                    chunk.iter_component::<C>(self.chunks_with_component.component),
                )
            })
    }

    /// Iterate as indexed, sliced, deserialized component batches.
    ///
    /// See [`re_chunk::Chunk::iter_slices`] for more information.
    pub fn slice<S: 'a + re_chunk::ChunkComponentSlicer>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), S::Item<'a>)> + 'a {
        self.chunks_with_component
            .chunks
            .iter()
            .flat_map(move |chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(
                        self.timeline,
                        self.chunks_with_component.component
                    ),
                    chunk.iter_slices::<S>(self.chunks_with_component.component),
                )
            })
    }

    /// Iterate as indexed, sliced, deserialized component batches for a specific struct field.
    ///
    /// See [`re_chunk::Chunk::iter_slices_from_struct_field`] for more information.
    pub fn slice_from_struct_field<S: 'a + re_chunk::ChunkComponentSlicer>(
        &'a self,
        field_name: &'a str,
    ) -> impl Iterator<Item = ((TimeInt, RowId), S::Item<'a>)> + 'a {
        self.chunks_with_component
            .chunks
            .iter()
            .flat_map(move |chunk| {
                itertools::izip!(
                    chunk.iter_component_indices(
                        self.timeline,
                        self.chunks_with_component.component
                    ),
                    chunk.iter_slices_from_struct_field::<S>(
                        self.chunks_with_component.component,
                        field_name
                    )
                )
            })
    }

    /// Access the underlying chunks.
    #[inline]
    pub fn chunks(&'a self) -> &'a ChunksWithComponent<'a> {
        &self.chunks_with_component
    }
}
