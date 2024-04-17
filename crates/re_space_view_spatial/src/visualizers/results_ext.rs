use std::borrow::Cow;

use re_data_store::RangeQuery;
use re_log_types::{RowId, TimeInt};
use re_query2::{PromiseResolver, PromiseResult};
use re_types::Component;

// --- Cached ---

use re_query_cache::{CachedLatestAtResults, CachedRangeData, CachedRangeResults};

pub trait CachedLatestAtResultsExt {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache::Result<Cow<'a, [C]>>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query_cache::Result<Cow<'a, [C]>>;
}

impl CachedLatestAtResultsExt for CachedLatestAtResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache::Result<Cow<'a, [C]>>> {
        let results = self.get(C::name())?;
        // TODO(#5607): what should happen if the promise is still pending?
        Some(match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(Cow::Borrowed(&[])),
            PromiseResult::Error(err) => Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        })
    }

    #[inline]
    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
    ) -> re_query_cache::Result<Cow<'a, [C]>> {
        let results = self.get_or_empty(C::name());
        // TODO(#5607): what should happen if the promise is still pending?
        match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(Cow::Borrowed(&[])),
            PromiseResult::Error(err) => Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        }
    }
}

pub trait CachedRangeResultsExt {
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> Option<re_query_cache::Result<CachedRangeData<'a, C>>>;

    fn get_or_empty_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> re_query_cache::Result<CachedRangeData<'a, C>>;
}

impl CachedRangeResultsExt for CachedRangeResults {
    #[inline]
    fn get_dense<'a, C: Component>(
        &'a self,
        resolver: &PromiseResolver,
        query: &RangeQuery,
    ) -> Option<re_query_cache::Result<CachedRangeData<'a, C>>> {
        let results = self.get(C::name())?.to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status(query.range());
        match front_status {
            PromiseResult::Error(err) => {
                return Some(Err(re_query_cache::QueryError::Other(err.into())))
            }
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => {
                return Some(Err(re_query_cache::QueryError::Other(err.into())))
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
    ) -> re_query_cache::Result<CachedRangeData<'a, C>> {
        let results = self.get_or_empty(C::name()).to_dense(resolver);

        // TODO(#5607): what should happen if the promise is still pending?
        let (front_status, back_status) = results.status(query.range());
        match front_status {
            PromiseResult::Error(err) => return Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }
        match back_status {
            PromiseResult::Error(err) => return Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Pending | PromiseResult::Ready(_) => {}
        }

        Ok(results)
    }
}

// --- Raw ---
//
// TODO(#5974): these APIs only exist because of all the unsolved caching issues we have, including
// the fact that we don't have any kind of garbage collection for caches.
// We cannot be having multiple copies of each image or the overhead would be unmanageable.

use re_query2::{LatestAtResults, RangeResults};

pub trait LatestAtResultsExt {
    fn get_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache::Result<Vec<C>>>;

    fn get_or_empty_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> re_query_cache::Result<Vec<C>>;
}

impl LatestAtResultsExt for LatestAtResults {
    #[inline]
    fn get_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<re_query_cache::Result<Vec<C>>> {
        let results = self.get(C::name())?;
        // TODO(#5607): what should happen if the promise is still pending?
        Some(match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(vec![]),
            PromiseResult::Error(err) => Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        })
    }

    #[inline]
    fn get_or_empty_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> re_query_cache::Result<Vec<C>> {
        let results = self.get_or_empty(C::name());
        // TODO(#5607): what should happen if the promise is still pending?
        match results.to_dense(resolver).flatten() {
            PromiseResult::Pending => Ok(vec![]),
            PromiseResult::Error(err) => Err(re_query_cache::QueryError::Other(err.into())),
            PromiseResult::Ready(data) => Ok(data),
        }
    }
}

pub trait RangeResultsExt {
    fn get_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<impl Iterator<Item = ((TimeInt, RowId), Vec<C>)>>;

    fn get_or_empty_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<C>)>;
}

impl RangeResultsExt for RangeResults {
    #[inline]
    fn get_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<impl Iterator<Item = ((TimeInt, RowId), Vec<C>)>> {
        let results = self.get(C::name())?;
        Some(
            itertools::izip!(results.iter_indices(), results.iter_dense::<C>(resolver))
                // TODO(#5607): what should happen if the promise is still pending?
                .filter_map(|(index, res)| match res.flatten() {
                    PromiseResult::Error(err) => {
                        // TODO: log real error
                        re_log::error!(%err, "xxx");
                        None
                    }
                    PromiseResult::Pending => None,
                    PromiseResult::Ready(data) => Some((index, data)),
                }),
        )
    }

    #[inline]
    fn get_or_empty_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> impl Iterator<Item = ((TimeInt, RowId), Vec<C>)> {
        let results = self.get_or_empty(C::name());
        itertools::izip!(results.iter_indices(), results.iter_dense::<C>(resolver))
            // TODO(#5607): what should happen if the promise is still pending?
            .filter_map(|(index, res)| match res.flatten() {
                PromiseResult::Error(err) => {
                    // TODO: log real error
                    re_log::error!(%err, "xxx");
                    None
                }
                PromiseResult::Pending => None,
                PromiseResult::Ready(data) => Some((index, data)),
            })
    }
}
