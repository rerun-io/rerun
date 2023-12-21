//! Caching datastructures for `re_query`.

mod cache;
mod flat_vec_deque;
mod query;

pub use self::cache::{AnyQuery, Caches};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::query::{
    query_cached_archetype_pov1, query_cached_archetype_pov1_comp1,
    query_cached_archetype_pov1_comp2, query_cached_archetype_pov1_comp3,
    query_cached_archetype_pov1_comp4, query_cached_archetype_pov1_comp5,
    query_cached_archetype_pov1_comp6, query_cached_archetype_pov1_comp7,
    query_cached_archetype_pov1_comp8, query_cached_archetype_pov1_comp9,
};

pub mod external {
    pub use re_query;
}
