use re_data_store::{LatestAtQuery, RangeQuery};
use re_log_types::{external::arrow2, RowId, TimeInt};
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
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
pub struct HybridLatestAtResults<'a> {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: LatestAtResults,
    pub ctx: &'a ViewContext<'a>,
    pub query: LatestAtQuery,
    pub data_result: &'a DataResult,
    pub resolver: PromiseResolver,
}

/// Wrapper that contains the results of a range query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridRangeResults {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: RangeResults,
}

impl<'a> HybridLatestAtResults<'a> {
    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> Option<&LatestAtComponentResults> {
        let component_name = component_name.into();
        if self.overrides.contains(component_name) {
            self.overrides.get(component_name)
        } else {
            self.results.get(component_name)
        }
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> re_query::Result<&LatestAtComponentResults> {
        let component_name = component_name.into();
        if self.overrides.contains(component_name) {
            self.overrides.get_required(component_name)
        } else {
            self.results.get_required(component_name)
        }
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_or_empty(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> &LatestAtComponentResults {
        let component_name = component_name.into();
        if self.overrides.contains(component_name) {
            self.overrides.get_or_empty(component_name)
        } else {
            self.results.get_or_empty(component_name)
        }
    }

    pub fn try_fallback_raw(
        &self,
        component_name: ComponentName,
    ) -> Option<Box<dyn arrow2::array::Array>> {
        let fallback_provider = self
            .data_result
            .best_fallback_for(self.ctx, component_name)?;

        let query_context = QueryContext {
            view_ctx: self.ctx,
            target_entity_path: &self.data_result.entity_path,
            archetype_name: None, // TODO(jleibs): Do we need this?
            query: &self.query,
        };

        fallback_provider
            .fallback_for(&query_context, component_name)
            .ok()
    }

    /// Utility for retrieving a single instance of a component.
    #[inline]
    pub fn get_instance<T: re_types_core::Component>(&self, index: usize) -> Option<T> {
        self.get(T::name())
            .and_then(|r| r.try_instance::<T>(&self.resolver, index))
    }

    /// Utility for retrieving a single instance of a component.
    #[inline]
    pub fn get_mono<T: re_types_core::Component>(&self) -> Option<T> {
        self.get_instance(0)
    }

    /// Utility for retrieving a single instance of a component with fallback
    #[inline]
    pub fn get_instance_with_fallback<T: re_types_core::Component + Default>(
        &self,
        index: usize,
    ) -> T {
        self.get(T::name())
            .and_then(|r| r.try_instance::<T>(&self.resolver, index))
            .or_else(|| {
                self.try_fallback_raw(T::name())
                    .and_then(|raw| T::from_arrow(raw.as_ref()).ok())
                    .and_then(|r| r.first().cloned())
            })
            .unwrap_or_default()
    }

    /// Utility for retrieving a single instance of a component.
    #[inline]
    pub fn get_mono_with_fallback<T: re_types_core::Component + Default>(&self) -> T {
        self.get_instance_with_fallback(0)
    }
}

pub enum HybridResults<'a> {
    LatestAt(LatestAtQuery, HybridLatestAtResults<'a>),
    Range(RangeQuery, HybridRangeResults),
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
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>>;
}

impl RangeResultsExt for Results {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'a, C>>> {
        match self {
            Self::LatestAt(_, results) => results.get_dense(resolver),
            Self::Range(_, results) => results.get_dense(resolver),
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
    fn get_dense<'a, C: Component>(
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
    fn get_dense<'a, C: Component>(
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
    fn get_dense<'a, C: Component>(
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
            self.results.get_dense(resolver)
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
            self.results.get_or_empty_dense(resolver)
        }
    }
}

impl<'a> RangeResultsExt for HybridLatestAtResults<'a> {
    #[inline]
    fn get_dense<'b, C: Component>(
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
            self.results.get_dense(resolver)
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
            self.results.get_or_empty_dense(resolver)
        }
    }
}

impl<'a> RangeResultsExt for HybridResults<'a> {
    fn get_dense<'b, C: Component>(
        &'b self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<RangeData<'b, C>>> {
        match self {
            Self::LatestAt(_, results) => results.get_dense(resolver),
            Self::Range(_, results) => results.get_dense(resolver),
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
