//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod flat_vec_deque;
mod latest_at;
mod range;

pub use self::cache::{CacheKey, Caches};
pub use self::cache_stats::{CachedComponentStats, CachesStats};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{
    CachedLatestAtComponentResults, CachedLatestAtMonoResult, CachedLatestAtResults,
};
pub use self::range::{CachedRangeComponentResults, CachedRangeData, CachedRangeResults};

pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::{CachedRangeComponentResultsInner, RangeCache};

pub use re_query2::{
    clamped_zip::*, range_zip::*, ExtraQueryHistory, Promise, PromiseId, PromiseResolver,
    PromiseResult, QueryError, Result, ToArchetype, VisibleHistory, VisibleHistoryBoundary,
};

pub mod external {
    pub use re_query2;

    pub use paste;
    pub use seq_macro;
}

// ---

#[inline]
pub fn is_component_cacheable(component_name: re_types_core::ComponentName) -> bool {
    use std::sync::OnceLock;

    use re_types_core::{ComponentNameSet, Loggable as _};

    static NEVER_CACHED: OnceLock<ComponentNameSet> = OnceLock::new();

    let never_cached = NEVER_CACHED.get_or_init(|| {
        ComponentNameSet::from([
            // TODO(#5303): InstanceKeys are on their way out.
            re_types::components::InstanceKey::name(), //
            // TODO(#5974): TensorData is already in the ad-hoc JPEG cache.
            re_types::components::TensorData::name(), //
            // TODO(#5974): MeshProperties is already in the ad-hoc Mesh cache.
            re_types::components::MeshProperties::name(), //
        ])
    });

    !never_cached.contains(&component_name)
}

// ---

use re_data_store::{LatestAtQuery, RangeQuery};

#[derive(Debug)]
pub enum CachedResults {
    LatestAt(LatestAtQuery, CachedLatestAtResults),
    Range(RangeQuery, CachedRangeResults),
}

impl From<(LatestAtQuery, CachedLatestAtResults)> for CachedResults {
    #[inline]
    fn from((query, results): (LatestAtQuery, CachedLatestAtResults)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl From<(RangeQuery, CachedRangeResults)> for CachedResults {
    #[inline]
    fn from((query, results): (RangeQuery, CachedRangeResults)) -> Self {
        Self::Range(query, results)
    }
}
