use re_query::{
    CachedLatestAtResults, CachedRangeData, CachedRangeResults, CachedResults, PromiseResolver,
    PromiseResult,
};
use re_types::Component;

// ---

/// Extension traits to abstract query result handling for all spatial space views.
///
/// Also turns all results into range results, so that views only have to worry about the ranged
/// case.
pub trait CachedRangeResultsExt {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<CachedRangeData<'a, C>>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<CachedRangeData<'a, C>>;
}

impl CachedRangeResultsExt for CachedResults {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<CachedRangeData<'a, C>>> {
        match self {
            CachedResults::LatestAt(_, results) => results.get_dense(resolver),
            CachedResults::Range(_, results) => results.get_dense(resolver),
        }
    }

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query::Result<CachedRangeData<'a, C>> {
        match self {
            CachedResults::LatestAt(_, results) => results.get_or_empty_dense(resolver),
            CachedResults::Range(_, results) => results.get_or_empty_dense(resolver),
        }
    }
}

impl CachedRangeResultsExt for CachedRangeResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<CachedRangeData<'a, C>>> {
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
    ) -> re_query::Result<CachedRangeData<'a, C>> {
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

impl CachedRangeResultsExt for CachedLatestAtResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query::Result<CachedRangeData<'a, C>>> {
        let results = self.get(C::name())?;
        let data = CachedRangeData::from_latest_at(resolver, results);

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
    ) -> re_query::Result<CachedRangeData<'a, C>> {
        let results = self.get_or_empty(C::name());
        let data = CachedRangeData::from_latest_at(resolver, results);

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
