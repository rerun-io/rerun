//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that match the specified entity path filter and contain
//! the target component.
//!
//! See [`Lens`] for more details and assumptions.

mod ast;
mod builder;
mod error;
mod op;
mod semantic;

pub use self::{
    ast::{Lens, Lenses, Op, OutputMode, PartialChunk},
    builder::{ColumnsBuilder, LensBuilder, ScatterColumnsBuilder, StaticColumnsBuilder},
    error::LensError,
    op::OpError,
};
