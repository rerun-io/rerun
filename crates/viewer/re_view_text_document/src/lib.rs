//! Rerun Text Document View
//!
//! A simple Viewshows a single text document.

#![warn(clippy::iter_over_hash_type)] //  TODO(#6198): enable everywhere

mod view_class;
mod visualizer_system;

pub use view_class::TextDocumentView;
