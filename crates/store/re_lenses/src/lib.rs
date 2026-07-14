//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that contain the target component.
//!
//! See [`Lens`] for more details and assumptions.

pub mod op;
mod runtime;

pub use self::runtime::default_runtime;

// Re-export the core lenses types.
pub use re_lenses_core::{
    CastTo, ChunkExt, DeriveLensBuilder, Lens, LensBuilderError, LensError, LensRuntimeError,
    Lenses, MutateLensBuilder, OutputMode, Runtime, Selector, function_registry,
};
