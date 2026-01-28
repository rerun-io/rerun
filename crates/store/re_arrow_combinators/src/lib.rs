//! Type-safe, composable transformations for Arrow arrays.
//!
//! This crate provides composable transformations for Arrow arrays.
//! Transformations are composable operations that convert one array type to another,
//! preserving structural properties like row counts and null handling.
//!
//! These transformations serve as building blocks for user-defined functions (UDFs)
//! in query engines like `DataFusion`, as well as SDK features like lenses.

mod error;
mod index;
mod transform;

pub mod cast;
pub mod map;
pub mod reshape;
mod selector;

pub use crate::cast::{DowncastRef, ListToFixedSizeList, PrimitiveCast};
pub use crate::error::Error;
pub use crate::selector::{Error as SelectorError, Selector, extract_nested_fields};
pub use crate::transform::{Compose, Transform};
