use std::borrow::Cow;
use std::sync::Arc;

use itertools::Itertools as _;
use nohash_hasher::IntMap;
use re_chunk_store::{LatestAtQuery, RangeQuery};
use re_log_types::hash::Hash64;
use re_query::{LatestAtResults, RangeResults};
use re_sdk_types::ComponentIdentifier;
use re_sdk_types::blueprint::datatypes::ComponentSourceKind;
use re_viewer_context::{DataResult, ViewContext, typed_fallback_for};

use crate::chunks_with_component::ChunksWithComponent;

// ---

/// Wrapper that contains the results of a latest-at query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
pub struct HybridLatestAtResults<'a> {
    pub overrides: LatestAtResults,
    pub store_results: LatestAtResults,
    pub view_defaults: &'a LatestAtResults,

    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,

    pub component_sources: IntMap<ComponentIdentifier, ComponentSourceKind>,

    /// Hash of mappings applied to [`Self::store_results`].
    pub component_indices_hash: Hash64,
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridRangeResults<'a> {
    pub(crate) overrides: LatestAtResults,
    pub(crate) store_results: RangeResults,
    pub(crate) view_defaults: &'a LatestAtResults,

    pub(crate) component_sources: IntMap<ComponentIdentifier, ComponentSourceKind>,

    /// Hash of mappings applied to [`Self::store_results`].
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
        &self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_>;

    /// Returns required component chunks with preserved store row IDs.
    ///
    /// Use this for required components where row IDs are needed for caching or identification.
    ///
    /// Blueprint row IDs are always discarded.
    #[inline]
    fn get_required_chunk(&self, component: ComponentIdentifier) -> ChunksWithComponent<'_> {
        self.get_chunks(component, true)
    }

    /// Returns optional component chunks with zeroed store row IDs.
    ///
    /// Use this for optional/recommended components where the original row IDs would otherwise
    /// interfere with range zipping on latest-at queries.
    ///
    /// Blueprint row IDs are always discarded.
    #[inline]
    fn get_optional_chunks(&self, component: ComponentIdentifier) -> ChunksWithComponent<'_> {
        self.get_chunks(component, false)
    }

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// **WARNING**: For latest-at queries, the row IDs are always zeroed out to allow for range zipping.
    /// Blueprint row IDs are always discarded.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::slice`]
    /// * [`HybridResultsChunkIter::slice_from_struct_field`]
    fn iter_as(
        &self,
        timeline: TimelineName,
        component: ComponentIdentifier,
    ) -> HybridResultsChunkIter<'_> {
        HybridResultsChunkIter {
            chunks_with_component: self.get_optional_chunks(component),
            timeline,
        }
    }
}

impl RangeResultsExt for LatestAtResults {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_> {
        let chunks = self.get(component).cloned().map_or_else(
            || Cow::Owned(vec![]),
            |chunk| {
                let mut chunk = Arc::unwrap_or_clone(chunk.into_chunk());
                if !force_preserve_store_row_ids {
                    chunk = chunk.into_static().zeroed();
                }
                Cow::Owned(vec![chunk])
            },
        );
        ChunksWithComponent { chunks, component }
    }
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        _force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_> {
        // Range queries always preserve row IDs
        let chunks = Cow::Borrowed(self.get(component).unwrap_or_default());
        ChunksWithComponent { chunks, component }
    }
}

impl RangeResultsExt for HybridRangeResults<'_> {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_> {
        let Some(source) = self.component_sources.get(&component) else {
            return ChunksWithComponent::empty(component);
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

                let results_chunks = self
                    .store_results
                    .get_chunks(component, force_preserve_store_row_ids);

                // TODO(cmc): this `collect_vec()` sucks, let's keep an eye on it and see if it ever
                // becomes an issue.
                Cow::Owned(
                    defaults
                        .into_iter()
                        .chain(results_chunks.chunks.iter().cloned())
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
            ComponentSourceKind::Fallback => Cow::Owned(Vec::new()),
        };

        ChunksWithComponent { chunks, component }
    }
}

impl RangeResultsExt for HybridLatestAtResults<'_> {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_> {
        let Some(source) = self.component_sources.get(&component) else {
            return ChunksWithComponent::empty(component);
        };

        let unit_chunk = match source {
            ComponentSourceKind::SourceComponent => {
                return self
                    .store_results
                    .get_chunks(component, force_preserve_store_row_ids);
            }
            ComponentSourceKind::Override => self.overrides.get(component),
            ComponentSourceKind::Default => self.view_defaults.get(component),
            ComponentSourceKind::Fallback => None,
        };

        if let Some(unit_chunk) = unit_chunk {
            // Because this is an override or default from the blueprint we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit_chunk.clone().into_chunk())
                .into_static()
                .zeroed();

            ChunksWithComponent {
                chunks: Cow::Owned(vec![chunk]),
                component,
            }
        } else {
            ChunksWithComponent::empty(component)
        }
    }
}

impl RangeResultsExt for HybridResults<'_> {
    #[inline]
    fn get_chunks(
        &self,
        component: ComponentIdentifier,
        force_preserve_store_row_ids: bool,
    ) -> ChunksWithComponent<'_> {
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

/// The iterator type backing [`HybridResults::iter_as`].
pub struct HybridResultsChunkIter<'a> {
    chunks_with_component: ChunksWithComponent<'a>,
    timeline: TimelineName,
}

impl<'a> HybridResultsChunkIter<'a> {
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
}
