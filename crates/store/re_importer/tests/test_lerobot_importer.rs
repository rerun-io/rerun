use std::sync::Arc;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_lerobot::{LeRobotDataset, common::LeRobotDatasetOps as _};
use re_log_types::StoreId;

const FIXTURES: [&str; 2] = ["v21_apple_storage", "v30_apple_storage"];

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets/lerobot")
        .join(name)
}

/// All chunks of all episodes, in load order.
fn load_all_chunks(fixture_name: &str) -> Vec<Chunk> {
    let dataset = LeRobotDataset::open(&fixture(fixture_name)).expect("fixture should open");
    dataset
        .iter_episode_indices()
        .flat_map(|episode| {
            dataset
                .load_episode_chunks(episode)
                .expect("fixture episode should load")
        })
        .collect()
}

/// Schema-level snapshot.
///
/// Episodes share one store here (each is its own recording in the real importer); for
/// schema purposes the union across episodes is what matters.
#[test]
fn test_lerobot_importer_schema() {
    for fixture_name in FIXTURES {
        let store_handle = ChunkStoreHandle::new(ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test_lerobot_importer"),
            ChunkStoreConfig::default(),
        ));

        {
            let mut store = store_handle.write();
            for chunk in load_all_chunks(fixture_name) {
                store.insert_chunk(&Arc::new(chunk)).unwrap();
            }
        }

        let schema = store_handle.read().schema().chunk_column_descriptors();
        insta::assert_debug_snapshot!(format!("{fixture_name}_schema"), schema);
    }
}
