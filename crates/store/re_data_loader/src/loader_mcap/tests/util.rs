//! Test utilities for MCAP data loader snapshot testing.

use std::path::{Path, PathBuf};

use re_chunk::{Chunk, EntityPath};

use crate::loader_mcap::McapLoader;
use crate::loader_mcap::lenses::foxglove_lenses;
use crate::{DataLoader as _, DataLoaderSettings, LoadedData};

// Helper function to get the path to a test asset file.
pub fn test_asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/loader_mcap/tests/assets")
        .join(name)
}

/// Loads an MCAP file using the default loader configuration for testing purposes.
pub fn load_mcap(path: impl AsRef<Path>) -> LoadedMcap {
    let path = path.as_ref();

    let loader = McapLoader::default().with_lenses(foxglove_lenses().unwrap());

    let (tx, rx) = crossbeam::channel::bounded(1024);
    let settings = DataLoaderSettings::recommended("test");

    loader
        .load_from_path(&settings, path.to_path_buf(), tx)
        .unwrap_or_else(|err| {
            panic!("Failed to load MCAP file at {}: {err}", path.display());
        });

    let chunks = rx
        .iter()
        .filter_map(|res| {
            if let LoadedData::Chunk(_, _, chunk) = res {
                Some(chunk)
            } else {
                None
            }
        })
        .collect();

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
