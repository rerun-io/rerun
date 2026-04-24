//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that contain the target component.
//!
//! See [`Lens`] for more details and assumptions.

pub mod op;

// Re-export core types for backward compatibility.
pub use re_lenses_core::{
    ChunkExt, Lens, LensBuilder, LensBuilderError, LensRuntimeError, Lenses, OutputBuilder,
    OutputMode, PartialChunk,
};
