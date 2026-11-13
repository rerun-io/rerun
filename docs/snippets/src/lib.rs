//! Snippets that we show in documentation.

#![expect(clippy::print_stdout, clippy::print_stderr)] // Snippets print what they do
#![expect(
    redundant_imports,
    reason = "Snippets import through `rerun::external` like users would, even when this crate has the dependency directly"
)]

mod snippets;

pub use snippets::run;
