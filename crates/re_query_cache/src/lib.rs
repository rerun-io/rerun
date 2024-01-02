//! Caching datastructures for `re_query`.

mod flat_vec_deque;

pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};

pub mod external {
    pub use re_query;
}
