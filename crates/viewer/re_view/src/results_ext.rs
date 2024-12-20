use std::{borrow::Cow, sync::Arc};

use itertools::Itertools as _;

use re_chunk_store::{Chunk, LatestAtQuery, RangeQuery, UnitChunkShared};
use re_log_types::external::arrow2::bitmap::Bitmap as Arrow2Bitmap;
use re_log_types::hash::Hash64;
use re_query::{LatestAtResults, RangeResults};
use re_types_core::ComponentName;
use re_viewer_context::{DataResult, QueryContext, ViewContext};

use crate::DataResultQuery as _;

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
    /// Returns the [`UnitChunkShared`] for the specified [`re_types_core::Component`].
    #[inline]
    pub fn get(&self, component_name: impl Into<ComponentName>) -> Option<&UnitChunkShared> {
        let component_name = component_name.into();
        self.overrides
            .get(&component_name)
            .or_else(|| self.results.get(&component_name))
            .or_else(|| self.defaults.get(&component_name))
    }

    pub fn fallback_raw(&self, component_name: ComponentName) -> arrow::array::ArrayRef {
        let query_context = QueryContext {
            viewer_ctx: self.ctx.viewer_ctx,
            target_entity_path: &self.data_result.entity_path,
            archetype_name: None, // TODO(jleibs): Do we need this?
            query: &self.query,
            view_state: self.ctx.view_state,
            view_ctx: Some(self.ctx),
        };

        self.data_result.best_fallback_for(
            &query_context,
            &self.ctx.visualizer_collection,
            component_name,
        )
    }

    /// Utility for retrieving the first instance of a component, ignoring defaults.
    #[inline]
    pub fn get_required_mono<C: re_types_core::Component>(&self) -> Option<C> {
        self.get_required_instance(0)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono<C: re_types_core::Component>(&self) -> Option<C> {
        self.get_instance(0)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono_with_fallback<C: re_types_core::Component + Default>(&self) -> C {
        self.get_instance_with_fallback(0)
    }

    /// Utility for retrieving a single instance of a component, not checking for defaults.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_required_instance<C: re_types_core::Component>(&self, index: usize) -> Option<C> {
        self.overrides.component_instance::<C>(index).or_else(||
                // No override -> try recording store instead
                self.results.component_instance::<C>(index))
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance<C: re_types_core::Component>(&self, index: usize) -> Option<C> {
        self.get_required_instance(index).or_else(|| {
            // No override & no store -> try default instead
            self.defaults.component_instance::<C>(index)
        })
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance_with_fallback<C: re_types_core::Component + Default>(
        &self,
        index: usize,
    ) -> C {
        self.get_instance(index)
            .or_else(|| {
                // No override, no store, no default -> try fallback instead
                let raw_fallback = self.fallback_raw(C::name());
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
                indices.extend(
                    r.results
                        .components
                        .iter()
                        .flat_map(|(component_name, chunks)| {
                            chunks
                                .iter()
                                .flat_map(|chunk| chunk.component_row_ids(component_name))
                        }),
                );

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
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>>;

    /// Returns component data for the given component or an empty array.
    ///
    /// For results that are aware of the blueprint, overrides, store results, and defaults will be
    /// considered.
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]>;

    /// Returns a zero-copy iterator over all the results for the given `(timeline, component)` pair.
    ///
    /// Call one of the following methods on the returned [`HybridResultsChunkIter`]:
    /// * [`HybridResultsChunkIter::primitive`]
    /// * [`HybridResultsChunkIter::primitive_array`]
    /// * [`HybridResultsChunkIter::string`]
    fn iter_as(
        &self,
        timeline: Timeline,
        component_name: ComponentName,
    ) -> HybridResultsChunkIter<'_> {
        let chunks = self.get_optional_chunks(&component_name);
        HybridResultsChunkIter {
            chunks,
            timeline,
            component_name,
        }
    }
}

impl RangeResultsExt for LatestAtResults {
    #[inline]
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        self.get(component_name)
            .cloned()
            .map(|chunk| Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]))
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        self.get(component_name).cloned().map_or_else(
            || Cow::Owned(vec![]),
            |chunk| Cow::Owned(vec![Arc::unwrap_or_clone(chunk.into_chunk())]),
        )
    }
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        self.get_required(component_name).ok().map(Cow::Borrowed)
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        Cow::Borrowed(self.get(component_name).unwrap_or_default())
    }
}

impl<'a> RangeResultsExt for HybridRangeResults<'a> {
    #[inline]
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        if let Some(unit) = self.overrides.get(component_name) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(Cow::Owned(vec![chunk]))
        } else {
            self.results.get_required_chunks(component_name)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        re_tracing::profile_function!();

        if let Some(unit) = self.overrides.get(component_name) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            re_tracing::profile_scope!("defaults");

            // NOTE: Because this is a range query, we always need the defaults to come first,
            // since range queries don't have any state to bootstrap from.
            let defaults = self.defaults.get(component_name).map(|unit| {
                // Because this is an default from the blueprint we always re-index the data as static
                Arc::unwrap_or_clone(unit.clone().into_chunk())
                    .into_static()
                    .zeroed()
            });

            let chunks = self.results.get_optional_chunks(component_name);

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
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        if let Some(unit) = self.overrides.get(component_name) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Some(Cow::Owned(vec![chunk]))
        } else {
            self.results.get_required_chunks(component_name)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        if let Some(unit) = self.overrides.get(component_name) {
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk())
                .into_static()
                .zeroed();
            Cow::Owned(vec![chunk])
        } else {
            let chunks = self
                .results
                .get_optional_chunks(component_name)
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
            let Some(unit) = self.defaults.get(component_name) else {
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
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        match self {
            Self::LatestAt(_, results) => results.get_required_chunks(component_name),
            Self::Range(_, results) => results.get_required_chunks(component_name),
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        match self {
            Self::LatestAt(_, results) => results.get_optional_chunks(component_name),
            Self::Range(_, results) => results.get_optional_chunks(component_name),
        }
    }
}

// ---

use re_chunk::{ChunkComponentIterItem, RowId, TimeInt, Timeline};
use re_chunk_store::external::{re_chunk, re_chunk::external::arrow2};

/// The iterator type backing [`HybridResults::iter_as`].
pub struct HybridResultsChunkIter<'a> {
    chunks: Cow<'a, [Chunk]>,
    timeline: Timeline,
    component_name: ComponentName,
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
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_component::<C>(),
            )
        })
    }

    /// Iterate as indexed booleans.
    ///
    /// See [`Chunk::iter_bool`] for more information.
    pub fn bool(&'a self) -> impl Iterator<Item = ((TimeInt, RowId), Arrow2Bitmap)> + 'a {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_bool(&self.component_name)
            )
        })
    }

    /// Iterate as indexed primitives.
    ///
    /// See [`Chunk::iter_primitive`] for more information.
    pub fn primitive<T: arrow2::types::NativeType>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), &'a [T])> + 'a {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_primitive::<T>(&self.component_name)
            )
        })
    }

    /// Iterate as indexed primitive arrays.
    ///
    /// See [`Chunk::iter_primitive_array`] for more information.
    pub fn primitive_array<const N: usize, T: arrow2::types::NativeType>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), &'a [[T; N]])> + 'a
    where
        [T; N]: bytemuck::Pod,
    {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_primitive_array::<N, T>(&self.component_name)
            )
        })
    }

    /// Iterate as indexed list of primitive arrays.
    ///
    /// See [`Chunk::iter_primitive_array_list`] for more information.
    pub fn primitive_array_list<const N: usize, T: arrow2::types::NativeType>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<&'a [[T; N]]>)> + 'a
    where
        [T; N]: bytemuck::Pod,
    {
        self.chunks.iter().flat_map(move |chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_primitive_array_list::<N, T>(&self.component_name)
            )
        })
    }

    /// Iterate as indexed UTF-8 strings.
    ///
    /// See [`Chunk::iter_string`] for more information.
    pub fn string(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<re_types_core::ArrowString>)> + 'a {
        self.chunks.iter().flat_map(|chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_string(&self.component_name)
            )
        })
    }

    /// Iterate as indexed buffers.
    ///
    /// See [`Chunk::iter_buffer`] for more information.
    pub fn buffer<T: arrow::datatypes::ArrowNativeType + arrow2::types::NativeType>(
        &'a self,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<re_types_core::ArrowBuffer<T>>)> + 'a {
        self.chunks.iter().flat_map(|chunk| {
            itertools::izip!(
                chunk.iter_component_indices(&self.timeline, &self.component_name),
                chunk.iter_buffer(&self.component_name)
            )
        })
    }
}
