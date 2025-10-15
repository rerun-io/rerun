//! Lenses allow you to extract, transform, and restructure component data. They
//! are applied to chunks that match the specified entity path filter and contain
//! the target component.
//!
//! See [`lenses::Lens`] for more details and assumptions. One way to make use of lenses is
//! by using the [`lenses::LensesSink`].

pub(crate) mod ast;
mod error;
mod op;
mod sink;

pub use self::{
    // We should be careful not to expose to much implementation details here.
    ast::{Lens, LensBuilder, Op},
    error::Error,
    sink::LensesSink,
};
