//! Core lens types and composable Arrow array transformations.
//!
//! This crate provides the Lenses definitions and builders, and composable
//! transformations for Arrow arrays. Transformations are operations convert
//! one array type to another, preserving structural properties like row
//! counts and null handling.

// Arrow `Transform` and combinators
pub mod combinators;

mod ast;
mod builder;
mod chunk;
mod error;
mod execute;
mod plan;
mod selector;

pub use self::{
    ast::{Lens, Lenses, OutputMode},
    builder::{DeriveLensBuilder, MutateLensBuilder},
    chunk::ChunkExt,
    error::{LensBuilderError, LensError, LensRuntimeError},
    selector::{
        DynExpr, Error as SelectorError, IntoDynExpr, Literal, Runtime, Selector,
        extract_nested_fields, function_registry,
    },
};
