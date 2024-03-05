//! Caching datastructures for `re_query`.

// TODO: we should keep instance keys around in tests -- let's not mix everything up
// TODO: let's ignore timeless until change it altogether?

// TODO: we need e2e examples that showcase (use 3d point cloud for both):
// - latest_at: query + clamped_zip
// - range: query + range_zip + clamped_zip

mod cache;
mod flat_vec_deque;
mod latest_at;
mod range;

pub use self::cache::{CacheKey, Caches};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{CachedLatestAtComponentResults, CachedLatestAtResults};
pub use self::range::{CachedRangeComponentResults, CachedRangeResults};

pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::RangeCache;

pub use re_query2::*; // convenience

pub mod external {
    pub use re_query2;

    pub use paste;
    pub use seq_macro;
}
