//! Implements the Rust codegen pass.

mod api;
mod arrow;
mod deserializer;
mod serializer;
mod util;

pub use self::api::RustCodeGenerator;
