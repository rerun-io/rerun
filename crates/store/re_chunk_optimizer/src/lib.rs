//! Analysis and memory-bounded optimization of Rerun chunk layouts.

mod analysis;
mod error;
mod executor;
mod optimize;
mod plan;
mod settings;
mod view;

pub use self::analysis::{ChunkIndexAnalysis, MergeAssessment, analyze_chunk_index};
pub use self::error::Error;
pub use self::optimize::optimize;
pub use self::settings::{MergeSplitSettings, OptimizationSettings};

/// For testing purposes only.
#[doc(hidden)]
pub mod testing {
    pub use super::executor::merge_split::{should_split_chunk, smallest_non_splitting_target};
}
