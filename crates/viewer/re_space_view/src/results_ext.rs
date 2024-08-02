use re_chunk_store::RowId;
use re_chunk_store::{LatestAtQuery, RangeQuery};
use re_log_types::hash::Hash64;
use re_log_types::{external::arrow2, TimeInt};
use re_query::{
    LatestAtComponentResults, LatestAtResults, PromiseResolver, PromiseResult, RangeData,
    RangeResults, Results,
};
use re_types_core::{Component, ComponentName};
use re_viewer_context::{DataResult, QueryContext, ViewContext};

use crate::DataResultQuery as _;

// ---

/// Wrapper that contains the results of a latest-at query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt2`] trait
/// they will be merged into the results appropriately.
pub struct HybridLatestAtResults<'a> {
    pub overrides: LatestAtResults,
    pub results: LatestAtResults,
    pub defaults: LatestAtResults,
    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,
    pub resolver: PromiseResolver,
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt2`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridRangeResults {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: RangeResults,
    pub(crate) defaults: LatestAtResults,
}

impl<'a> HybridLatestAtResults<'a> {
    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> Option<&LatestAtComponentResults> {
        let component_name = component_name.into();
        self.overrides
            .get(component_name)
            .or_else(|| self.results.get(component_name))
            .or_else(|| self.defaults.get(component_name))
    }

    pub fn try_fallback_raw(
        &self,
        component_name: ComponentName,
    ) -> Option<Box<dyn arrow2::array::Array>> {
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
    pub fn get_required_mono<T: re_types_core::Component>(&self) -> Option<T> {
        self.get_required_instance(0)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono<T: re_types_core::Component>(&self) -> Option<T> {
        self.get_instance(0)
    }

    /// Utility for retrieving the first instance of a component.
    #[inline]
    pub fn get_mono_with_fallback<T: re_types_core::Component + Default>(&self) -> T {
        self.get_instance_with_fallback(0)
    }

    /// Utility for retrieving a single instance of a component, not checking for defaults.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_required_instance<T: re_types_core::Component>(&self, index: usize) -> Option<T> {
        let component_name = T::name();

        self.overrides
            .get(component_name)
            .and_then(|r| r.try_instance::<T>(&self.resolver, index))
            .or_else(||
                // No override -> try recording store instead
                self.results
                    .get(component_name)
                    .and_then(|r| r.try_instance::<T>(&self.resolver, index)))
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance<T: re_types_core::Component>(&self, index: usize) -> Option<T> {
        self.get_required_instance(index).or_else(|| {
            // No override & no store -> try default instead
            self.defaults
                .get(T::name())
                .and_then(|r| r.try_instance::<T>(&self.resolver, index))
        })
    }

    /// Utility for retrieving a single instance of a component.
    ///
    /// If overrides or defaults are present, they will only be used respectively if they have a component at the specified index.
    #[inline]
    pub fn get_instance_with_fallback<T: re_types_core::Component + Default>(
        &self,
        index: usize,
    ) -> T {
        self.get_instance(index)
            .or_else(|| {
                // No override, no store, no default -> try fallback instead
                self.try_fallback_raw(T::name())
                    .and_then(|raw| T::from_arrow(raw.as_ref()).ok())
                    .and_then(|r| r.first().cloned())
            })
            .unwrap_or_default()
    }
}

pub enum HybridResults<'a> {
    LatestAt(LatestAtQuery, HybridLatestAtResults<'a>),
    Range(RangeQuery, HybridRangeResults),
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
                indices.extend(r.defaults.components.values().map(|r| *r.index()));
                indices.extend(r.overrides.components.values().map(|r| *r.index()));
                indices.extend(r.results.components.values().map(|r| *r.index()));

                Hash64::hash(&indices)
            }
            Self::Range(_, r) => {
                let mut indices = Vec::with_capacity(
                    r.defaults.components.len()
                        + r.overrides.components.len()
                        + r.results.components.len(), // Don't know how many results per component.
                );
                indices.extend(r.defaults.components.values().map(|r| *r.index()));
                indices.extend(r.overrides.components.values().map(|r| *r.index()));
                indices.extend(r.results.components.values().flat_map(|r| {
                    // Have top collect in order to release the lock.
                    r.read().indices().copied().collect::<Vec<_>>()
                }));

                Hash64::hash(&indices)
            }
        }
    }
}

impl<'a> From<(LatestAtQuery, HybridLatestAtResults<'a>)> for HybridResults<'a> {
    #[inline]
    fn from((query, results): (LatestAtQuery, HybridLatestAtResults<'a>)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl<'a> From<(RangeQuery, HybridRangeResults)> for HybridResults<'a> {
    #[inline]
    fn from((query, results): (RangeQuery, HybridRangeResults)) -> Self {
        Self::Range(query, results)
    }
}

/// Extension traits to abstract query result handling for all spatial space views.
///
/// Also turns all results into range results, so that views only have to worry about the ranged
/// case.
pub trait RangeResultsExt {
    /// Returns dense component data for the given component, ignores default data if the result distinguishes them.
    ///
    /// For results that are aware of the blueprint, only overrides & store results will be considered.
    /// Defaults have no effect.
    fn get_required_component_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>>;

    /// Returns dense component data for the given component or an empty array.
    ///
    /// For results that are aware of the blueprint, overrides, store results, and defaults will be considered.
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>>;
}

impl RangeResultsExt for Results {
    fn get_required_component_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>> {
        match self {
            Self::LatestAt(_, results) => results.get_required_component_dense(resolver),
            Self::Range(_, results) => results.get_required_component_dense(resolver),
        }
    }

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>> {
        match self {
            Self::LatestAt(_, results) => results.get_or_empty_dense(resolver),
            Self::Range(_, results) => results.get_or_empty_dense(resolver),
        }
    }
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_required_component_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>> {
        let results = self.get(C::name())?.to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status();
        match front_status {
            PromiseResult::Error(err) => return Some(Err(re_query::QueryError::Other(err.into()))),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => return Some(Err(re_query::QueryError::Other(err.into()))),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Some(Ok(results))
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>> {
        let results = self.get_or_empty(C::name()).to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status();
        match front_status {
            PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Ok(results)
    }
}

impl RangeResultsExt for LatestAtResults {
    #[inline]
    fn get_required_component_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>> {
        let results = self.get(C::name())?;
        let data = RangeData::from_latest_at(resolver, results, None);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = data.status();
        match front_status {
            PromiseResult::Error(err) => return Some(Err(re_query::QueryError::Other(err.into()))),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => return Some(Err(re_query::QueryError::Other(err.into()))),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Some(Ok(data))
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>> {
        let results = self.get_or_empty(C::name());
        // With latest-at semantics, we just want to join the secondary components onto the primary
        // ones, irrelevant of their indices.
        // In particular, it is pretty common to have a secondary component be more recent than the
        // associated primary component in latest-at contexts, e.g. colors in an otherwise fixed
        // point cloud being changed each frame.
        let data =
            RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = data.status();
        match front_status {
            PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Ok(data)
    }
}

impl RangeResultsExt for HybridRangeResults {
    #[inline]
    fn get_required_component_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>> {
        let component_name = C::name();

        if self.overrides.contains(component_name) {
            let results = self.overrides.get(C::name())?;
            // Because this is an override we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => {
                    return Some(Err(re_query::QueryError::Other(err.into())))
                }
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => {
                    return Some(Err(re_query::QueryError::Other(err.into())))
                }
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Some(Ok(data))
        } else {
            self.results.get_required_component_dense(resolver)
        }
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>> {
        let component_name = C::name();

        if self.overrides.contains(component_name) {
            let results = self.overrides.get_or_empty(C::name());
            // Because this is an override we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Ok(data)
        } else {
            let data = self.results.get_or_empty_dense(resolver);

            // If the data is not empty, return it.
            if let Ok(data) = data {
                if !data.is_empty() {
                    return Ok(data);
                }
            };

            // Otherwise try to use the default data.

            let results = self.defaults.get_or_empty(C::name());
            // Because this is an default from the blueprint we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Ok(data)
        }
    }
}

impl<'a> RangeResultsExt for HybridLatestAtResults<'a> {
    #[inline]
    fn get_required_component_dense<'b, C: Component>(
        &'b self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'b, C>>> {
        let component_name = C::name();

        if self.overrides.contains(component_name) {
            let results = self.overrides.get(C::name())?;
            // Because this is an override we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => {
                    return Some(Err(re_query::QueryError::Other(err.into())))
                }
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => {
                    return Some(Err(re_query::QueryError::Other(err.into())))
                }
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Some(Ok(data))
        } else {
            self.results.get_required_component_dense(resolver)
        }
    }

    #[inline]
    fn get_or_empty_dense<'b, C: Component>(
        &'b self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'b, C>> {
        let component_name = C::name();

        if self.overrides.contains(component_name) {
            let results = self.overrides.get_or_empty(C::name());

            // Because this is an override we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Ok(data)
        } else {
            let data = self.results.get_or_empty_dense(resolver);

            // If the data is not empty, return it.
            if let Ok(data) = data {
                if !data.is_empty() {
                    return Ok(data);
                }
            };

            // Otherwise try to use the default data.

            let results = self.defaults.get_or_empty(C::name());
            // Because this is an default from the blueprint we always re-index the data as static
            let data =
                RangeData::from_latest_at(resolver, results, Some((TimeInt::STATIC, RowId::ZERO)));

            // TODO(#5607): what should happen if the promise is still pending?
            let (front_status, back_status) = data.status();
            match front_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }
            match back_status {
                PromiseResult::Error(err) => return Err(re_query::QueryError::Other(err.into())),
                PromiseResult::Pending | PromiseResult::Ready(_) => {}
            }

            Ok(data)
        }
    }
}

impl<'a> RangeResultsExt for HybridResults<'a> {
    fn get_required_component_dense<'b, C: Component>(
        &'b self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'b, C>>> {
        match self {
            Self::LatestAt(_, results) => results.get_required_component_dense(resolver),
            Self::Range(_, results) => results.get_required_component_dense(resolver),
        }
    }

    fn get_or_empty_dense<'b, C: Component>(
        &'b self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'b, C>> {
        match self {
            Self::LatestAt(_, results) => results.get_or_empty_dense(resolver),
            Self::Range(_, results) => results.get_or_empty_dense(resolver),
        }
    }
}
