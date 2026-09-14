//! Rerun importer for MCAP files.

mod importer;
mod robot_description;

/// Lens implementations for transforming various third-party data formats into Rerun components.
pub mod lenses;

#[cfg(test)]
pub mod tests;

pub use importer::McapImporter;
pub use lenses::FOXGLOVE_LENSES_IDENTIFIER;
