//! Implements the Rust codegen pass.

mod api;
mod arrow;
mod deserializer;
mod reflection;
mod serializer;
mod util;

pub use self::api::RustCodeGenerator;
