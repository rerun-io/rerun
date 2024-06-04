use re_data_store::{LatestAtQuery, RangeQuery};
use re_log_types::{RowId, TimeInt};
use re_query::{
    LatestAtComponentResults, LatestAtResults, PromiseResolver, PromiseResult, RangeData,
    RangeResults, Results,
};
use re_types_core::{Component, ComponentName};

// ---

/// Wrapper that contains the results of a latest-at query with possible overrides.
///
/// Although overrides are never temporal, when accessed via the [`crate::RangeResultsExt`] trait
/// they will be merged into the results appropriately.
#[derive(Debug)]
pub struct HybridLatestAtResults {
    pub(crate) overrides: LatestAtResults,
    pub(crate) results: LatestAtResults,
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

impl HybridLatestAtResults {
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
}

#[derive(Debug)]
pub enum HybridResults {
    LatestAt(LatestAtQuery, HybridLatestAtResults),
    Range(RangeQuery, HybridRangeResults),
}

impl From<(LatestAtQuery, HybridLatestAtResults)> for HybridResults {
    #[inline]
    fn from((query, results): (LatestAtQuery, HybridLatestAtResults)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl From<(RangeQuery, HybridRangeResults)> for HybridResults {
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
            Self::LatestAt(_, results) => RangeResultsExt::get_dense(results, resolver),
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

impl RangeResultsExt for HybridLatestAtResults {
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
            RangeResultsExt::get_dense(&self.results, resolver)
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

impl RangeResultsExt for HybridResults {
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
