//! The vocabulary that Rerun's IDL definitions are written against.
//!
//! Rerun's type definitions are a subset of Rust that is parsed by `re_types_builder` and *also*
//! compiled by rustc, purely so that we get name resolution, typo-checking, rust-analyzer and
//! `cargo fmt` for free. The definitions crate is never linked into anything.
//!
//! Definitions refer to each other by their fully-qualified name with `.` swapped for `::`
//! — `rerun.components.Position3D` is written `rerun::components::Position3D` — and never
//! contain a `use` statement. A single `extern crate self as rerun;` in the definitions crate's
//! generated `lib.rs` makes that resolve in every module.
//!
//! For that rule to hold without exceptions, every name a definition can mention has to live
//! under `rerun::`, including the two that have no spelling in plain Rust. Hence this crate:
//! the definitions crate re-exports [`Binary`] and [`struct@f16`] at its own root, so they are
//! written `rerun::Binary` and `rerun::f16` like everything else.

pub use half::f16;

pub use re_types_builder_macros::rerun_type;

/// A list of bytes of arbitrary length — the Arrow `Binary` type.
///
/// Written `rerun::Binary` in a definition. This is a name for the frontend to recognize, not a
/// type anyone constructs; the generated code uses the target language's own byte-buffer type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Binary;
