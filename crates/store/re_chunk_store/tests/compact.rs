//! Tests for `compacted()` and `finalize_compaction`.

#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, CompactionOptions};
use re_log_types::example_components::{MyPoint, MyPoints};
use re_log_types::{EntityPath, TimePoint, Timeline};

/// Builds a store with many single-row chunks sharing entity `/sensor` and
/// timeline `"frame"`. Intentionally fragmented to trigger compaction.
fn fragmented_store() -> ChunkStore {
    let store_id = re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app");
    let mut store = ChunkStore::new(store_id, ChunkStoreConfig::ALL_DISABLED);

    let entity_path: EntityPath = "/sensor".into();
    let timeline_frame = Timeline::new_sequence("frame");

    for i in 0..20 {
        let timepoint = TimePoint::from_iter([(timeline_frame, i as i64)]);
        let point = MyPoint::new(i as f32, i as f32);
        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(
                RowId::new(),
                timepoint,
                (MyPoints::descriptor_points(), &[point]),
            )
            .build()
            .expect("build chunk");
        store.insert_chunk(&Arc::new(chunk)).expect("insert chunk");
    }

    store
}

fn options(num_extra_passes: Option<usize>) -> CompactionOptions {
    CompactionOptions {
        config: ChunkStoreConfig::DEFAULT,
        num_extra_passes,
        is_start_of_gop: None,
    }
}

#[test]
fn compacted_reduces_chunk_count() {
    let store = fragmented_store();
    let before = store.num_physical_chunks();
    let compacted = store.compacted(&options(Some(50))).expect("compacted");
    assert!(compacted.num_physical_chunks() < before);
}

#[test]
fn finalize_compaction_converges() {
    let store = fragmented_store()
        .compacted(&options(Some(50)))
        .expect("initial");
    let before = store.num_physical_chunks();
    let store2 = store
        .finalize_compaction(&options(Some(5)))
        .expect("idempotent");
    assert_eq!(before, store2.num_physical_chunks());
}

#[test]
fn compacted_preserves_row_count() {
    let store = fragmented_store();
    let rows_before: u64 = store
        .iter_physical_chunks()
        .map(|c| c.num_rows() as u64)
        .sum();
    let compacted = store.compacted(&options(Some(50))).expect("compacted");
    let rows_after: u64 = compacted
        .iter_physical_chunks()
        .map(|c| c.num_rows() as u64)
        .sum();
    assert_eq!(rows_before, rows_after);
}
