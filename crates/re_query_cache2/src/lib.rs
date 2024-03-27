//! Caching datastructures for `re_query`.

mod cache;
mod flat_vec_deque;
mod latest_at;
mod range;

pub use self::cache::{CacheKey, Caches};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{
    CachedLatestAtComponentResults, CachedLatestAtMonoResult, CachedLatestAtResults,
};
pub use self::range::{CachedRangeComponentResults, CachedRangeResults};

pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::{CachedRangeComponentResultsInner, RangeCache};

pub use re_query2::{
    clamped_zip::*, range_zip::*, Promise, PromiseId, PromiseResolver, PromiseResult, QueryError,
    Result, ToArchetype,
};

pub mod external {
    pub use re_query2;

    pub use paste;
    pub use seq_macro;
}
