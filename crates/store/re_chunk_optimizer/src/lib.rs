//! Analysis and memory-bounded optimization of Rerun chunk layouts.

mod analysis;
mod error;
mod view;

pub use self::analysis::{ChunkIndexAnalysis, MergeAssessment, analyze_chunk_index};
pub use self::error::Error;
