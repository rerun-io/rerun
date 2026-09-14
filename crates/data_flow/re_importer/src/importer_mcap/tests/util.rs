//! Test utilities for MCAP importer snapshot testing.

use std::collections::BTreeMap;
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

/// An individual test's configuration.
struct McapTest {
    /// MCAP file name (relative to the `assets/` directory).
    mcap_file: &'static str,

    /// Entity path of the message to snapshot (i.e. the channel name in MCAP).
    entity_path: &'static str,
}

/// Runs MCAP importer snapshot tests.
pub struct McapTestHarness {
    tests: Vec<McapTest>,
}

impl McapTestHarness {
    pub fn new() -> Self {
        Self { tests: Vec::new() }
    }

    /// Register a snapshot test case for an entity imported from an MCAP file.
    pub fn add(mut self, mcap_file: &'static str, entity_path: &'static str) -> Self {
        self.tests.push(McapTest {
            mcap_file,
            entity_path,
        });
        self
    }

    /// Runs snapshot tests for all registered MCAP files and their entities.
    ///
    /// Imports the MCAP and snapshots the chunk of each entity to be tested.
    pub fn run(self) {
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path("snapshots");
        settings.set_prepend_module_to_snapshot(false);

        settings.bind(|| {
            // An input MCAP file may be used in multiple tests.
            // Group all tests that come from the same input file in one snapshot.
            let mut tests_by_mcap = BTreeMap::<_, Vec<_>>::new();
            for test in self.tests {
                tests_by_mcap.entry(test.mcap_file).or_default().push(test);
            }

            for (mcap_file, tests) in tests_by_mcap {
                let asset_path = test_asset(mcap_file);
                let loaded_mcap = load_mcap(&asset_path);
                let snapshot_name = Path::new(mcap_file)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .expect("MCAP test assets must have a UTF-8 file stem");
                let snapshot = tests
                    .into_iter()
                    .map(|test| {
                        // The first chunk contains metadata and the second contains the imported payload.
                        let chunks = loaded_mcap.chunks_for_entity(test.entity_path);
                        let chunk = chunks.get(1).unwrap_or_else(|| {
                            panic!(
                                "Expected a metadata chunk followed by a payload chunk at entity {}\nFile path: {}",
                                test.entity_path,
                                asset_path.display(),
                            )
                        });
                        format!("{chunk:-240}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let mut snapshot_settings = insta::Settings::clone_current();
                snapshot_settings.set_input_file(&asset_path);
                snapshot_settings.bind(|| {
                    insta::assert_snapshot!(snapshot_name, snapshot);
                });
            }
        });
    }
}

fn load_mcap(path: impl AsRef<Path>) -> LoadedMcap {
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

    LoadedMcap { chunks }
}

struct LoadedMcap {
    chunks: Vec<Chunk>,
}

impl LoadedMcap {
    fn chunks_for_entity(&self, path: &str) -> Vec<&Chunk> {
        let entity_path: EntityPath = path.into();
        self.chunks
            .iter()
            .filter(|c| c.entity_path() == &entity_path)
            .collect()
    }
}
