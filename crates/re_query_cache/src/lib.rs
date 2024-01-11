//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod flat_vec_deque;
mod latest_at;
mod query;

pub use self::cache::{AnyQuery, Caches};
pub use self::cache_stats::{
    detailed_stats, set_detailed_stats, CachedComponentStats, CachedEntityStats, CachesStats,
};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::query::{
    query_archetype_pov1, query_archetype_with_history_pov1, MaybeCachedComponentData,
};
seq_macro::seq!(NUM_COMP in 0..10 { paste::paste! {
    pub use self::query::{#(
        query_archetype_pov1_comp~NUM_COMP,
        query_archetype_with_history_pov1_comp~NUM_COMP,
    )*};
}});

pub(crate) use self::cache::CacheBucket;
pub(crate) use self::latest_at::LatestAtCache;
seq_macro::seq!(NUM_COMP in 0..10 { paste::paste! {
    pub(crate) use self::latest_at::{#(
        query_archetype_latest_at_pov1_comp~NUM_COMP,
    )*};
}});

pub use re_query::{QueryError, Result}; // convenience

pub mod external {
    pub use re_query;

    pub use paste;
    pub use seq_macro;
}
