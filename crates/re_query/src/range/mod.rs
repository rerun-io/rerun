mod query;
mod results;

pub use self::query::RangeCache;
pub use self::results::{
    CachedRangeComponentResults, CachedRangeComponentResultsInner, CachedRangeData,
    CachedRangeResults,
};
