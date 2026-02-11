use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk_store::external::re_chunk::external::arrow::array::ArrayRef;
use re_chunk_store::{LatestAtQuery, RangeQuery, UnitChunkShared};
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
    pub(crate) store_results: LatestAtResults,
    pub view_defaults: &'a LatestAtResults,
    pub(crate) instruction_id: Option<VisualizerInstructionId>,

    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,

    pub(crate) component_sources:
        IntMap<ComponentIdentifier, Result<ComponentSourceKind, ComponentMappingError>>,

    /// Hash of mappings applied to [`Self::store_results`].
    pub(crate) component_indices_hash: Hash64,
}

impl<'a> BlueprintResolvedLatestAtResults<'a> {
    /// Are there any chunks that need to be fetched from a remote store?
    pub fn any_missing_chunks(&self) -> bool {
        0 < self.overrides.missing_virtual.len()
            + self.store_results.missing_virtual.len()
            + self.view_defaults.missing_virtual.len()
    }

    /// Returns the [`UnitChunkShared`] for the given component, respecting overrides, store results, and defaults.
    ///
    /// `force_preserve_store_row_ids`: If true, preserves row IDs from store data.
    /// If false, results are re-indexed to static with zeroed row IDs to allow range zipping.
    /// Blueprint data (overrides/defaults) is **always** re-indexed regardless of this setting.
    pub fn get_unit_chunk(
        &'a self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> Result<Option<Cow<'a, UnitChunkShared>>, ComponentMappingError> {
        let Some(source) = self.component_sources.get(&component) else {
            return Ok(None);
        };
        let source = source.clone()?;

        let blueprint_unit_chunk = match source {
            ComponentSourceKind::SourceComponent => {
                if let Some(unit_chunk) = self.store_results.get(component) {
                    let unit_chunk = if force_preserve_store_row_ids {
                        Cow::Borrowed(unit_chunk)
                    } else {
                        let chunk: re_chunk::Chunk = (**unit_chunk).clone();
                        let chunk = chunk.into_static().zeroed();
                        Cow::Owned(chunk.into_unit().expect(
                            "This was a unit chunk to begin with, so converting it back can't fail",
                        ))
                    };
                    return Ok(Some(unit_chunk));
                } else {
                    None
                }
            }
            ComponentSourceKind::Override => self.overrides.get(component),
            ComponentSourceKind::Default => self.view_defaults.get(component),
        };

        if let Some(unit_chunk) = blueprint_unit_chunk {
            // Because this is an override or default from the blueprint we always re-index the data as static
            let chunk: re_chunk::Chunk = (**unit_chunk).clone();
            let chunk = chunk.into_static().zeroed();

            let unit_chunk =
                Cow::Owned(chunk.into_unit().expect(
                    "This was a unit chunk to begin with, so converting it back can't fail",
                ));

            Ok(Some(unit_chunk))
        } else {
            Ok(None)
        }
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
        // Merge component source from bootstrap into the range results.
        #[expect(clippy::iter_over_hash_type)] // Fills up another hash type.
        for (component, bootstrap_source) in bootstrapped.component_sources {
            match self.component_sources.entry(component) {
                std::collections::hash_map::Entry::Occupied(mut range_query_source) => {
                    #[expect(clippy::match_same_arms)]
                    match bootstrap_source {
                        Ok(_) => {
                            // Don't override the source, let the range result take precedence.
                        }

                        Err(ComponentMappingError::ComponentNotFound(_)) => {
                            // Component wasn't found in the bootstrap data.
                            // Data may only exist within the range actual range, if not it has the error already!
                        }

                        Err(
                            ComponentMappingError::SelectorParseFailed(_)
                            | ComponentMappingError::SelectorExecutionFailed(_)
                            | ComponentMappingError::CastFailed { .. },
                        ) => {
                            // All these errors are likely to occur in the range query as well.
                            // However, in case they don't we treat it as-if we failed first and then never executed the range query.
                            // I.e. override what's there!
                            *range_query_source.get_mut() = bootstrap_source;
                        }
                    }
                }

                std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                    // Data showed up only in the target range result.
                    // That's unusual since we'd expect the same query on bootstrapped & the target range result,
                    // but sure enough we can just combine in the bootstrap!
                    vacant_entry.insert(bootstrap_source);
                }
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
    /// Utility for retrieving the first instance of a component.
    ///
    /// This operates on a single component at a time and does not handle
    /// required vs. optional component semantics needed when zipping multiple components together.
    /// For multi-component zipping, see [`crate::VisualizerInstructionQueryResults`] which properly handles
    /// required vs. optional distinction.
    #[inline]
    pub fn get_mono<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> Option<C> {
        // We don't care about row ids here, but preserving means less overhead!
        let force_preserve_store_row_ids = true;
        let unit_chunk = self
            .get_unit_chunk(component, force_preserve_store_row_ids)
            .ok()??;

        let deserialized_row = unit_chunk.iter_component::<C>(component).next()?;
        deserialized_row.as_slice().first().cloned()
    }

    /// Utility for retrieving the first instance of a component, falling back to the registered fallback value.
    ///
    /// This operates on a single component at a time and does not handle
    /// required vs. optional component semantics needed when zipping multiple components together.
    /// For multi-component zipping, see [`crate::VisualizerInstructionQueryResults`] which properly handles
    /// required vs. optional distinction.
    #[inline]
    pub fn get_mono_with_fallback<C: re_types_core::Component>(
        &self,
        component: ComponentIdentifier,
    ) -> C {
        self.get_mono::<C>(component).unwrap_or_else(|| {
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

    /// Returns the raw arrow array for the given component's single cell.
    ///
    /// This operates on a single component at a time and does not handle
    /// required vs. optional component semantics needed when zipping multiple components together.
    /// For multi-component zipping, see [`crate::VisualizerInstructionQueryResults`] which properly handles
    /// required vs. optional distinction.
    #[inline]
    pub fn get_raw_cell(&self, component: ComponentIdentifier) -> Option<ArrayRef> {
        // We don't care about row ids here, but preserving means less overhead!
        let force_preserve_store_row_ids = true;
        let unit_chunk = self
            .get_unit_chunk(component, force_preserve_store_row_ids)
            .ok()??;

        unit_chunk.component_batch_raw(component)
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
        match self.get_unit_chunk(component, force_preserve_store_row_ids) {
            Ok(None) => MaybeChunksWithComponent::empty(component),

            Ok(Some(unit_chunk)) => MaybeChunksWithComponent {
                maybe_chunks: Ok(match unit_chunk {
                    Cow::Borrowed(unit_chunk) => Cow::Borrowed(std::slice::from_ref(unit_chunk)),
                    // TODO(andreas): Would be nice to get rid of the extra clone and vec allocation here.
                    Cow::Owned(unit_chunk) => Cow::Owned(vec![(*unit_chunk.into_chunk()).clone()]),
                }),
                component,
            },

            Err(err) => MaybeChunksWithComponent::error(component, err),
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
