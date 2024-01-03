//! Caching datastructures for `re_query`.

mod cache;
mod flat_vec_deque;
mod query;

pub use self::cache::{AnyQuery, Caches};
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::query::{query_cached_archetype_pov1, query_cached_archetype_with_history_pov1};

// TODO(cmc): Supporting N>1 generically is quite painful due to limitations in declarative macros,
// not that we care at the moment.
seq_macro::seq!(NUM_COMP in 0..10 { paste::paste! {
    pub use self::query::{#(
        query_cached_archetype_pov1_comp~NUM_COMP,
        query_cached_archetype_with_history_pov1_comp~NUM_COMP,
    )*};
}});

pub mod external {
    pub use re_query;
}
