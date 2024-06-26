//! Implements the Rust codegen pass.

mod api;
mod arrow;
mod blueprint_validation;
mod deserializer;
mod reflection;
mod serializer;
mod util;

pub use self::api::RustCodeGenerator;
