//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that match the specified entity path filter and contain
//! the target component.
//!
//! See [`Lens`] for more details and assumptions.

pub mod op;

// Re-export core types for backward compatibility.
pub use re_lenses_core::{
    ColumnsBuilder, Lens, LensBuilder, LensError, Lenses, OutputMode, PartialChunk,
    ScatterColumnsBuilder, StaticColumnsBuilder,
};
