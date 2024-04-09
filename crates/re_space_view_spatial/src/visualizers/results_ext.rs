use re_data_store::RangeQuery;
use re_query_cache2::{
    CachedLatestAtResults, CachedRangeData, CachedRangeResults, PromiseResolver, PromiseResult,
};
use re_types::Component;

// ---

pub trait CachedLatestAtResultsExt {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache2::Result<&'a [C]>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query_cache2::Result<&'a [C]>;
}

impl CachedLatestAtResultsExt for CachedLatestAtResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache2::Result<&'a [C]>> {
        let results = self.get(C::name())?;
        // TODO(#5607): what should happen if the promise is still pending?
        Some(match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(&[]),
            PromiseResult::Error(err) => Err(re_query_cache2::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        })
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query_cache2::Result<&'a [C]> {
        let results = self.get_or_empty(C::name());
        // TODO(#5607): what should happen if the promise is still pending?
        match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(&[]),
            PromiseResult::Error(err) => Err(re_query_cache2::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        }
    }
}

pub trait CachedRangeResultsExt {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> Option<re_query_cache2::Result<CachedRangeData<'a, C>>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> re_query_cache2::Result<CachedRangeData<'a, C>>;
}

impl CachedRangeResultsExt for CachedRangeResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> Option<re_query_cache2::Result<CachedRangeData<'a, C>>> {
        let results = self.get(C::name())?.to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status(query.range());
        match front_status {
            PromiseResult::Error(err) => {
                return Some(Err(re_query_cache2::QueryError::Other(err.into())))
            }
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => {
                return Some(Err(re_query_cache2::QueryError::Other(err.into())))
            }
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Some(Ok(results))
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> re_query_cache2::Result<CachedRangeData<'a, C>> {
        let results = self.get_or_empty(C::name()).to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status(query.range());
        match front_status {
            PromiseResult::Error(err) => {
                return Err(re_query_cache2::QueryError::Other(err.into()))
            }
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => {
                return Err(re_query_cache2::QueryError::Other(err.into()))
            }
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Ok(results)
    }
}
