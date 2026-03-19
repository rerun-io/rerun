//! Core lens types and composable Arrow array transformations.
//!
//! This crate provides the Lenses definitions and builders, and composable
//! transformations for Arrow arrays. Transformations are composable operations
//! that convert one array type to another, preserving structural properties
//! like row counts and null handling.

// Arrow `Transform` and combinators
pub mod combinators;

// Selector
mod selector;

pub use crate::selector::{
    Error as SelectorError, Literal, Runtime, Selector, extract_nested_fields, function_registry,
};

// Lenses
mod ast;
mod builder;
mod lens_error;

pub use self::{
    ast::{Lens, Lenses, OutputMode, PartialChunk},
    builder::{ColumnsBuilder, LensBuilder, ScatterColumnsBuilder, StaticColumnsBuilder},
    lens_error::LensError,
};
