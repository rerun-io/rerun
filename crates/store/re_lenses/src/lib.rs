//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that contain the target component.
//!
//! See [`Lens`] for more details and assumptions.

pub mod op;

// Re-export the core lenses types.
pub use re_lenses_core::{
    ChunkExt, DeriveLensBuilder, Lens, LensBuilderError, LensError, LensRuntimeError, Lenses,
    MutateLensBuilder, OutputMode,
};
