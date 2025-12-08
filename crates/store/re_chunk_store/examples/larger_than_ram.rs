use itertools::Itertools as _;
use re_chunk::{RangeQuery, Timeline};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_types::{AbsoluteTimeRange, StoreKind, TimeType};
use re_query::RangeResults;
use re_sdk_types::{Archetype, archetypes::Boxes3D};

fn main() {
    let nuscenes_rrd = "/home/cmc/dev/rerun-io/rerun/nuscenes.rrd";
    let stores =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::COMPACTION_DISABLED, nuscenes_rrd)
            .unwrap();

    let recordings = stores
        .into_values()
        .filter(|s| s.id().kind() == StoreKind::Recording)
        .collect_vec();
    assert!(recordings.len() == 1);

    let mut store = recordings.into_iter().next().unwrap();
    dbg!(store.num_chunks());

    let static_chunk_ids = store
        .iter_chunks()
        .filter(|c| c.is_static())
        .map(|c| c.id())
        .collect_vec();

    let temporal_chunk_ids = store
        .iter_chunks()
        .filter(|c| !c.is_static())
        .map(|c| c.id())
        .collect_vec();

    {
        for id in &static_chunk_ids {
            assert!(store.chunk(id).is_some());
        }
        for id in &temporal_chunk_ids {
            assert!(store.chunk(id).is_some());
        }
    }

    let store_handle = ChunkStoreHandle::new(store);
    let caches = re_query::QueryCache::new(store_handle.clone());

    let entity_path = "/world/anns";
    // let entity_path = "/world/ego_vehicle/trajectory";
    // let timeline = Timeline::log_time();
    let timeline = Timeline::new_timestamp("timestamp");
    let query = RangeQuery::new(*timeline.name(), AbsoluteTimeRange::EVERYTHING);
    eprintln!("query:{query:?}");

    {
        let results: RangeResults = caches.range(
            &query,
            &entity_path.into(),
            Boxes3D::all_component_identifiers(),
        );
        eprintln!(
            "query found {} actual chunks and {} chunks that need to be downloaded",
            results
                .components
                .values()
                .map(|chunks| chunks.len())
                .sum::<usize>(),
            results.missing.len(),
        );
    }

    if true {
        let mut store = store_handle.write();
        // TODO: what about STATIC
        store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        dbg!(store.num_chunks());

        {
            for id in &static_chunk_ids {
                assert!(store.chunk(id).is_some());
            }
            for id in &temporal_chunk_ids {
                assert!(store.chunk(id).is_none()); // ⚠️is_none()
            }
        }
    }

    // First, get the (potentially cached) results for this query.
    {
        let results: RangeResults = caches.range(
            &query,
            &entity_path.into(),
            Boxes3D::all_component_identifiers(),
        );
        eprintln!(
            "query found {} actual chunks and {} chunks that need to be downloaded",
            results
                .components
                .values()
                .map(|chunks| chunks.len())
                .sum::<usize>(),
            results.missing.len(),
        );
    }

    for chunk in store_handle.read().iter_chunks() {
        assert!(chunk.is_static());
    }
}
