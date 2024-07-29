use std::borrow::Cow;
use std::sync::Arc;

use re_chunk_store::{Chunk, LatestAtQuery, RangeQuery};
use re_log_types::external::arrow2::array::Array as ArrowArray;
use re_log_types::hash::Hash64;
use re_query2::{LatestAtResults, RangeResults};
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
    pub defaults: LatestAtResults,

    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridRangeResults {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: RangeResults,
    pub(crate) defaults: LatestAtResults,
}

impl<'a> HybridLatestAtResults<'a> {
    pub fn try_fallback_raw(&self, component_name: ComponentName) -> Option<Box<dyn ArrowArray>> {
        let fallback_provider = self
            .data_result
            .best_fallback_for(self.ctx, component_name)?;

        let query_context = QueryContext {
            viewer_ctx: self.ctx.viewer_ctx,
            target_entity_path: &self.data_result.entity_path,
            archetype_name: None, // TODO(jleibs): Do we need this?
            query: &self.query,
            view_state: self.ctx.view_state,
            view_ctx: Some(self.ctx),
        };

        fallback_provider
            .fallback_for(&query_context, component_name)
            .ok()
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
                self.try_fallback_raw(C::name())
                    .and_then(|raw| C::from_arrow(raw.as_ref()).ok())
                    .and_then(|r| r.first().cloned())
            })
            .unwrap_or_default()
    }
}

pub enum HybridResults<'a> {
    LatestAt(LatestAtQuery, HybridLatestAtResults<'a>),

    // Boxed because of size difference between variants
    Range(RangeQuery, Box<HybridRangeResults>),
}

impl<'a> HybridResults<'a> {
    pub fn query_result_hash(&self) -> Hash64 {
        re_tracing::profile_function!();
        // TODO(andreas): We should be able to do better than this and determine hashes for queries on the fly.

        match self {
            Self::LatestAt(_, r) => {
                let mut indices = Vec::with_capacity(
                    r.defaults.components.len()
                        + r.overrides.components.len()
                        + r.results.components.len(),
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
                    r.defaults.components.len()
                        + r.overrides.components.len()
                        + r.results.components.len(), // Don't know how many results per component.
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

impl<'a> From<(RangeQuery, HybridRangeResults)> for HybridResults<'a> {
    #[inline]
    fn from((query, results): (RangeQuery, HybridRangeResults)) -> Self {
        Self::Range(query, Box::new(results))
    }
}

/// Extension traits to abstract query result handling for all spatial space views.
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

impl RangeResultsExt for HybridRangeResults {
    #[inline]
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        if self.overrides.contains(component_name) {
            let unit = self.overrides.get(component_name)?;
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Some(Cow::Owned(vec![chunk]))
        } else {
            self.results.get_required_chunks(component_name)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        if self.overrides.contains(component_name) {
            let Some(unit) = self.overrides.get(component_name) else {
                return Cow::Owned(Vec::new());
            };
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Cow::Owned(vec![chunk])
        } else {
            let chunks = self.results.get_optional_chunks(component_name);

            // If the data is not empty, return it.

            if !chunks.is_empty() {
                return chunks;
            }

            // Otherwise try to use the default data.

            let Some(unit) = self.defaults.get(component_name) else {
                return Cow::Owned(Vec::new());
            };
            // Because this is an default from the blueprint we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Cow::Owned(vec![chunk])
        }
    }
}

impl<'a> RangeResultsExt for HybridLatestAtResults<'a> {
    #[inline]
    fn get_required_chunks(&self, component_name: &ComponentName) -> Option<Cow<'_, [Chunk]>> {
        if self.overrides.contains(component_name) {
            let unit = self.overrides.get(component_name)?;
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Some(Cow::Owned(vec![chunk]))
        } else {
            self.results.get_required_chunks(component_name)
        }
    }

    #[inline]
    fn get_optional_chunks(&self, component_name: &ComponentName) -> Cow<'_, [Chunk]> {
        if self.overrides.contains(component_name) {
            let Some(unit) = self.overrides.get(component_name) else {
                return Cow::Owned(Vec::new());
            };
            // Because this is an override we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Cow::Owned(vec![chunk])
        } else {
            let chunks = self.results.get_optional_chunks(component_name);

            // If the data is not empty, return it.

            if !chunks.is_empty() {
                return chunks;
            }

            // Otherwise try to use the default data.

            let Some(unit) = self.defaults.get(component_name) else {
                return Cow::Owned(Vec::new());
            };
            // Because this is an default from the blueprint we always re-index the data as static
            let chunk = Arc::unwrap_or_clone(unit.clone().into_chunk()).into_static();
            Cow::Owned(vec![chunk])
        }
    }
}

impl<'a> RangeResultsExt for HybridResults<'a> {
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
