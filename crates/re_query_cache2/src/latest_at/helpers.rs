use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::{EntityPath, RowId, TimeInt};
use re_types_core::Component;

use crate::{Caches, PromiseResolver, PromiseResult};

// ---

#[derive(Clone)]
pub struct CachedLatestAtMonoResult<C> {
    pub index: (TimeInt, RowId),
    pub value: C,
}

impl<C> CachedLatestAtMonoResult<C> {
    #[inline]
    pub fn data_time(&self) -> TimeInt {
        self.index.0
    }

    #[inline]
    pub fn row_id(&self) -> RowId {
        self.index.1
    }
}

impl<C: std::ops::Deref> std::ops::Deref for CachedLatestAtMonoResult<C> {
    type Target = C;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// ---

impl Caches {
    /// Get the latest index and value for a given dense [`re_types_core::Component`].
    ///
    /// Returns `None` if the data is a promise that has yet to be resolved.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will generate a log message of `level` otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log messages on failure.
    //
    // TODO: issue about figuring out what to do when these things are pending
    pub fn latest_at_component_with_log_level<C: Component>(
        &self,
        store: &DataStore,
        resolver: &PromiseResolver,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
        level: re_log::Level,
    ) -> Option<CachedLatestAtMonoResult<C>> {
        re_tracing::profile_function!();

        let results = self.latest_at(store, query, entity_path, [C::name()]);
        let result = results.get::<C>()?;

        let index @ (data_time, row_id) = *result.index();

        match result.to_dense::<C>(resolver).flatten() {
            PromiseResult::Pending => {
                re_log::debug_once!(
                    "Couldn't deserialize {entity_path}:{} @ {data_time:?}#{row_id}: promise still pending",
                    C::name(),
                );
                None
            }
            PromiseResult::Ready(data) if data.len() == 1 => Some(CachedLatestAtMonoResult {
                index,
                value: data[0].clone(),
            }),
            PromiseResult::Ready(data) => {
                re_log::log_once!(
                    level,
                    "Couldn't deserialize {entity_path}:{} @ {data_time:?}#{row_id}: not a mono-batch (length: {})",
                    C::name(),
                    data.len(),
                );
                None
            }
            PromiseResult::Error(err) => {
                re_log::log_once!(
                    level,
                    "Couldn't deserialize {entity_path} @ {data_time:?}#{row_id}:{}: {}",
                    C::name(),
                    re_error::format_ref(&*err),
                );
                None
            }
        }
    }

    /// Get the latest index and value for a given dense [`re_types_core::Component`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    #[inline]
    pub fn query_latest_component<C: Component>(
        &self,
        store: &DataStore,
        resolver: &PromiseResolver,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<CachedLatestAtMonoResult<C>> {
        self.latest_at_component_with_log_level(
            store,
            resolver,
            entity_path,
            query,
            re_log::Level::Warn,
        )
    }

    /// Get the latest index and value for a given dense [`re_types_core::Component`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will return None and log a debug message otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely logs debug messages on failure.
    #[inline]
    pub fn query_latest_component_quiet<C: Component>(
        &self,
        store: &DataStore,
        resolver: &PromiseResolver,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<CachedLatestAtMonoResult<C>> {
        self.latest_at_component_with_log_level(
            store,
            resolver,
            entity_path,
            query,
            re_log::Level::Debug,
        )
    }

    /// Call [`Self::query_latest_component`] at the given path, walking up the hierarchy until an instance is found.
    pub fn query_latest_component_at_closest_ancestor<C: Component>(
        &self,
        store: &DataStore,
        resolver: &PromiseResolver,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(EntityPath, CachedLatestAtMonoResult<C>)> {
        re_tracing::profile_function!();

        let mut cur_entity_path = Some(entity_path.clone());
        while let Some(entity_path) = cur_entity_path {
            if let Some(result) =
                self.query_latest_component::<C>(store, resolver, &entity_path, query)
            {
                return Some((entity_path, result));
            }
            cur_entity_path = entity_path.parent();
        }
        None
    }
}
