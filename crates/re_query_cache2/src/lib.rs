//! Caching datastructures for `re_query`.

mod cache;
mod flat_vec_deque;
mod latest_at;

pub use self::cache::{CacheKey, Caches};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{
    CachedLatestAtComponentResults, CachedLatestAtMonoResult, CachedLatestAtResults,
};

pub(crate) use self::latest_at::LatestAtCache;

pub use re_query2::{
    clamped_zip::*, Promise, PromiseId, PromiseResolver, PromiseResult, QueryError, Result,
    ToArchetype,
};

pub mod external {
    pub use re_query2;

    pub use paste;
    pub use seq_macro;
}
