//! Test utilities for MCAP importer snapshot testing.

use std::path::{Path, PathBuf};

use re_chunk::{Chunk, EntityPath};

use crate::importer_mcap::McapImporter;
use crate::{ImportedData, Importer as _, ImporterSettings};

// Helper function to get the path to a test asset file.
pub fn test_asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/importer_mcap/tests/assets")
        .join(name)
}

/// Loads an MCAP file using the default importer configuration for testing purposes.
pub fn load_mcap(path: impl AsRef<Path>) -> LoadedMcap {
    let path = path.as_ref();

    let importer = McapImporter::default();

    let (tx, rx) = crossbeam::channel::bounded(1024);
    let settings = ImporterSettings::recommended("test");

    importer
        .import_from_path(&settings, path.to_path_buf(), tx)
        .unwrap_or_else(|err| {
            panic!("Failed to load MCAP file at {}: {err}", path.display());
        });

    let chunks: Vec<Chunk> = rx.iter().filter_map(ImportedData::into_chunk).collect();

    if 10_000 < chunks.len() {
        re_log::warn!(
            "MCAP file contained {} chunks. Consider running `rerun rrd optimize` on the output.",
            re_format::format_uint(chunks.len()),
        );
    }

    LoadedMcap { chunks }
}

/// Result of loading a test MCAP file, providing convenient access to chunks.
pub struct LoadedMcap {
    chunks: Vec<Chunk>,
}

impl LoadedMcap {
    /// Returns chunks for a specific entity path.
    pub fn chunks_for_entity(&self, path: &str) -> Vec<&Chunk> {
        let entity_path: EntityPath = path.into();
        self.chunks
            .iter()
            .filter(|c| c.entity_path() == &entity_path)
            .collect()
    }
}
