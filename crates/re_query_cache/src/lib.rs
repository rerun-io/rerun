//! Caching datastructures for `re_query`.

// TODO: standard GC is our biggest problem right now ><

// TODO: shall we bite the bullet and import concat_indents? looked very verbose

// TODO: next but not quite sure when:
// - example pushing to many plots (100kHz single plot and 100kHz distributed over 25 plots)
//    - rrd, py, rs, cpp
// - aggregation
//
//
// TODO: next:
// - bucket merge & only return results in range
// - error handling / QueryError... or even QueryCacheError?
// - timeless support
// - point2d?
// - sizebytes everywhere
// - sizebytes in cachestats + ui view
// - invalidation / storeview + tests
// - FlatVecDeque benchmarks
// - high-level tests
// - copy re_query benchmarks in cached mode
// - LRU behavior + --query-cache-size-limit + tests

// TODO: multi tenant caching has to come in a second wave

// TODO: welp, we're doomed for stats: need all components to impl SizeBytes :/
// ...although, aren't all components PODs, in a way?

// TODO: caching toggle, dump stats, cache everywhere

// TODO: should just remove the non-cached path, shouldnt we?

// TODO: garbage collection is no different than invalidation

// TODO: PRs:
// - FlatVecDeque + re_query_cache skeleton
// - QueryCache without memlimit/invalidation
// - query_with_history stuff
// - SizeBytes for everything + SizeBytes POD?
// - Component: SizeBytes?
// - cache + tests
// - cache memstats
// - cached time series
// - cached text logs
// - cached range utils (everything, essentially -> only point clouds for now (2D too?))
//
// to be sorted:
// - batched store events for additions
// - new batch GC

// TODO: arranging the caches by hash(Query) is nice and all but what happens when someone's play
// around with the time range UI? It's not the same query anymore...
// That's not necessarily something that needs to work at first though.

// TODO: are per-view indices really that important? yes in a way, but also we can ship a first
// version without and go a long way.

// TODO: revive https://github.com/sebosp/swarmy once we're done here!

// TODO: we need a bucket system for the range stuff...

// TODO: are we really going to duplicate the cache for each timeline tho? dont really have much
// chance considering we're caching joins and everything
// -> and even per archetype now!

mod cache;
mod flat_vec_deque;
mod query;

pub use self::cache::{AnyQuery, CachedQueryResult, Caches};
pub use self::flat_vec_deque::FlatVecDeque;
pub use self::query::{
    query_cached_archetype_r1, query_cached_archetype_r1o1, query_cached_archetype_r1o2,
    query_cached_archetype_r1o3, query_cached_archetype_r1o4, query_cached_archetype_r1o5,
    query_cached_archetype_r1o6, query_cached_archetype_r1o7, query_cached_archetype_r1o8,
    query_cached_archetype_r1o9, query_cached_archetype_with_history_r1,
    query_cached_archetype_with_history_r1o1, query_cached_archetype_with_history_r1o2,
    query_cached_archetype_with_history_r1o3, query_cached_archetype_with_history_r1o4,
    query_cached_archetype_with_history_r1o5, query_cached_archetype_with_history_r1o6,
    query_cached_archetype_with_history_r1o7, query_cached_archetype_with_history_r1o8,
    query_cached_archetype_with_history_r1o9,
};

pub(crate) use self::cache::CACHES;

pub mod external {
    pub use re_query;
}
