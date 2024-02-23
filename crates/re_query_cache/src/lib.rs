//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod flat_vec_deque;
mod latest_at;
mod query;
mod range;

pub use self::cache::{AnyQuery, Caches};
pub use self::cache_stats::{
    CachedComponentStats, CachedEntityStats, CachesStats, CachesStatsKind,
};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::query::iter_or_repeat_opt;

pub(crate) use self::cache::CacheBucket;
pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::RangeCache;

pub use re_query::{QueryError, Result}; // convenience

pub mod external {
    pub use re_query;

    pub use paste;
    pub use seq_macro;
}
