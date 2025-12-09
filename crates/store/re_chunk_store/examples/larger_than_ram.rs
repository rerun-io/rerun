use itertools::Itertools as _;

use re_chunk::{LatestAtQuery, RangeQuery, TimeInt, Timeline};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, ChunkStoreHandle};
use re_log_encoding::RrdManifest;
use re_log_types::{AbsoluteTimeRange, StoreKind};
use re_query::{LatestAtResults, QueryCache, RangeResults};
use re_sdk_types::{Archetype, archetypes::Boxes3D};

fn main() {
    // TODO: this is the future!!1!

    let nuscenes_rrd = "/home/cmc/dev/rerun-io/rerun/nuscenes.rrd";
    let rrd_bytes = std::fs::read(nuscenes_rrd).unwrap();
    let rrd_manifest = RrdManifest::from_rrd_bytes(&rrd_bytes).unwrap().unwrap();
    // dbg!(rrd_manifest.to_native_static().unwrap());
    // dbg!(rrd_manifest.to_native_temporal().unwrap());
    let store = ChunkStore::from_rrd_manifest(&rrd_manifest).unwrap();

    let store_handle = ChunkStoreHandle::new(store);
    let caches = re_query::QueryCache::new(store_handle.clone());

    do_latestat_query(&caches);
    do_range_query(&caches);

    eprintln!("------");
    eprintln!("------");
    eprintln!("------");
    eprintln!("------");

    // ---

    let nuscenes_rrd = "/home/cmc/dev/rerun-io/rerun/nuscenes.rrd";
    let stores =
        ChunkStore::from_rrd_filepath(&ChunkStoreConfig::COMPACTION_DISABLED, nuscenes_rrd)
            .unwrap();

    let recordings = stores
        .into_values()
        .filter(|s| s.id().kind() == StoreKind::Recording)
        .collect_vec();
    assert!(recordings.len() == 1);

    let store = recordings.into_iter().next().unwrap();

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

    do_latestat_query(&caches);
    do_range_query(&caches);

    if true {
        let mut store = store_handle.write();
        // TODO: what about STATIC

        let num_chunks_before_gc = store.num_chunks();
        for _ in 0..5 {
            store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        }
        let num_chunks_after_gc = store.num_chunks();

        eprintln!(
            "\n⚠️ Running garbage collection:\n* {num_chunks_before_gc} chunks before\n* {num_chunks_after_gc} chunks after\n"
        );

        {
            for id in &static_chunk_ids {
                assert!(store.chunk(id).is_some());
            }
            for id in &temporal_chunk_ids {
                assert!(store.chunk(id).is_none()); // ⚠️is_none()
            }
        }

        for chunk in store.iter_chunks() {
            assert!(chunk.is_static());
        }
    }

    do_latestat_query(&caches);
    do_range_query(&caches);
}

fn do_latestat_query(caches: &QueryCache) {
    let entity_path = "/world/anns";
    // let entity_path = "/world/ego_vehicle/trajectory";
    // let timeline = Timeline::log_time();
    let timeline = Timeline::new_timestamp("timestamp");
    let query = LatestAtQuery::new(*timeline.name(), TimeInt::MAX);
    // eprintln!("query:{query:?}");

    {
        let results: LatestAtResults = caches.latest_at(
            &query,
            &entity_path.into(),
            Boxes3D::all_component_identifiers(),
        );
        // dbg!(results.components.values().map(|c| c.id()).collect_vec());
        assert!(results.components.values().all(|chunk| !chunk.is_static()));
        eprintln!(
            "LatestAt query found {} actual chunks and {} chunks that need to be downloaded",
            results.components.len(),
            results.missing_chunk_ids.len(),
        );
    }
}

fn do_range_query(caches: &QueryCache) {
    let entity_path = "/world/anns";
    // let entity_path = "/world/ego_vehicle/trajectory";
    // let timeline = Timeline::log_time();
    let timeline = Timeline::new_timestamp("timestamp");
    let query = RangeQuery::new(*timeline.name(), AbsoluteTimeRange::EVERYTHING);
    // eprintln!("query:{query:?}");

    {
        let results: RangeResults = caches.range(
            &query,
            &entity_path.into(),
            Boxes3D::all_component_identifiers(),
        );
        assert!(
            results
                .components
                .values()
                .flatten()
                .all(|chunk| !chunk.is_static())
        );
        eprintln!(
            "Range query found {} actual chunks and {} chunks that need to be downloaded",
            results
                .components
                .values()
                .map(|chunks| chunks.len())
                .sum::<usize>(),
            results.missing_chunk_ids.len(),
        );
    }
}
