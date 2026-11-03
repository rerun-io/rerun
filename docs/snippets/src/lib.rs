//! Snippets that we show in documentation.

#![expect(clippy::print_stdout, clippy::print_stderr)] // Snippets print what they do

mod snippets;

pub use snippets::run;
