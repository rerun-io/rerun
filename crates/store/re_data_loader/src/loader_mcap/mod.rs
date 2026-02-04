//! Rerun dataloader for MCAP files.

mod loader;

/// Lens implementations for transforming various third-party data formats into Rerun components.
pub mod lenses;

pub use loader::{McapLoader, load_mcap};
