//! Rerun dataloader for MCAP files.

mod loader;

/// Lens implementations for transforming various third-party data formats into Rerun components.
pub mod lenses;

#[cfg(test)]
pub mod tests;

pub use lenses::FOXGLOVE_LENSES_IDENTIFIER;
pub use loader::{McapLoader, load_mcap};
