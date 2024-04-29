use re_log_types::{RowId, TimeInt};
use re_query::{LatestAtResults, PromiseResolver, PromiseResult, RangeData, RangeResults, Results};
use re_types::Component;

// ---

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
            Results::LatestAt(_, results) => results.get_dense(resolver),
            Results::Range(_, results) => results.get_dense(resolver),
        }
    }

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<RangeData<'a, C>> {
        match self {
            Results::LatestAt(_, results) => results.get_or_empty_dense(resolver),
            Results::Range(_, results) => results.get_or_empty_dense(resolver),
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
        let data = RangeData::from_latest_at(resolver, results, Some((TimeInt::MIN, RowId::ZERO)));

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
