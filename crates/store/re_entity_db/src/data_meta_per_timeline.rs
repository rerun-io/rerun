use std::collections::BTreeMap;

use re_chunk::TimelineName;
use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};

use crate::RrdManifestIndex;

#[derive(Default, Clone, Copy)]
struct RowCount {
    /// Row count from a rrd manifest.
    from_rrd_manifest: u64,

    /// Row counts from volatile chunks, i.e chunks that aren't in a rrd manifest.
    from_volatile_chunks: u64,
}

impl RowCount {
    fn is_empty(&self) -> bool {
        self.from_rrd_manifest == 0 && self.from_volatile_chunks == 0
    }
}

impl re_byte_size::SizeBytes for RowCount {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            from_rrd_manifest: _,
            from_volatile_chunks: _,
        } = self;

        0
    }

    fn is_pod() -> bool {
        true
    }
}

/// Helper to track row counts and time ranges across all entities and components per timeline.
#[derive(Default, Clone)]
pub struct DataMetaPerTimeline {
    row_count_per_timeline: BTreeMap<TimelineName, RowCount>,
}

impl re_byte_size::SizeBytes for DataMetaPerTimeline {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            row_count_per_timeline,
        } = self;

        row_count_per_timeline.heap_size_bytes()
    }
}

impl DataMetaPerTimeline {
    pub fn row_count_for_timeline(&self, timeline: &TimelineName) -> u64 {
        self.row_count_per_timeline
            .get(timeline)
            .map(
                |RowCount {
                     from_rrd_manifest,
                     from_volatile_chunks,
                 }| from_rrd_manifest + from_volatile_chunks,
            )
            .unwrap_or(0)
    }

    fn handle_event_for_row_count(
        &mut self,
        manifest_index: &RrdManifestIndex,
        store: &ChunkStore,
        event: &ChunkStoreEvent,
    ) {
        match &event.diff {
            ChunkStoreDiff::Addition(addition) => {
                // If this addition comes from a root chunk in the rrd manifest,
                // then don't count it since we've already counted that with a virtual
                // addition.
                if store
                    .find_root_rrd_manifests(&addition.delta_chunk().id())
                    .is_empty()
                {
                    for (timeline, col) in addition.delta_chunk().timelines() {
                        let row_count = self.row_count_per_timeline.entry(*timeline).or_default();

                        row_count.from_volatile_chunks += col.num_rows() as u64;
                    }
                }
            }
            ChunkStoreDiff::VirtualAddition(addition) => {
                for per_timeline in addition.rrd_manifest.temporal_map().values() {
                    for (timeline, per_component) in per_timeline {
                        let row_count = self
                            .row_count_per_timeline
                            .entry(*timeline.name())
                            .or_default();

                        for chunks in per_component.values() {
                            for entry in chunks.values() {
                                row_count.from_rrd_manifest += entry.num_rows;
                            }
                        }
                    }
                }
            }
            ChunkStoreDiff::Deletion(deletion) => {
                let mut rrd_manifest_row_counts = BTreeMap::new();

                // We don't want to subtract rows that were in the rrd manifest
                // since those are tracked separately and never deleted.
                //
                // So we collect the count of all root chunks in the rrd manifest
                // for the deleted chunk.
                let rrd_manifest_row_counts_iter = store
                    .find_root_rrd_manifests(&deletion.chunk.id())
                    .into_iter()
                    .filter_map(|(c, _)| manifest_index.root_chunk_info(&c))
                    .flat_map(|info| {
                        info.temporals.iter().map(|(timeline, info)| {
                            (*timeline, info.num_rows_for_all_entities_all_components)
                        })
                    });

                for (timeline, row_count) in rrd_manifest_row_counts_iter {
                    *rrd_manifest_row_counts.entry(timeline).or_insert(0) += row_count;
                }

                for (timeline, col) in deletion.chunk.timelines() {
                    let row_count = self.row_count_per_timeline.entry(*timeline).or_default();

                    let chunk_volatile_chunk_count = (col.num_rows() as u64).saturating_sub(
                        rrd_manifest_row_counts.get(timeline).copied().unwrap_or(0),
                    );

                    row_count.from_volatile_chunks = row_count
                        .from_volatile_chunks
                        .saturating_sub(chunk_volatile_chunk_count);

                    if row_count.is_empty() {
                        self.row_count_per_timeline.remove(timeline);
                    }
                }
            }
        }
    }

    pub fn on_events(
        &mut self,
        manifest_index: &RrdManifestIndex,
        store: &ChunkStore,
        events: &[ChunkStoreEvent],
    ) {
        re_tracing::profile_function!();

        for event in events {
            self.handle_event_for_row_count(manifest_index, store, event);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk::{Chunk, RowId, TimelineName};
    use re_chunk_store::{ChunkStore, ChunkStoreConfig};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{EntityPath, StoreId, TimePoint, Timeline};
    use re_types_core::ComponentBatch;

    use super::*;
    use crate::RrdManifestIndex;

    /// Insert a single-row chunk and feed the resulting events into `meta`.
    fn insert_and_update(
        store: &mut ChunkStore,
        meta: &mut DataMetaPerTimeline,
        manifest: &RrdManifestIndex,
        entity: &EntityPath,
        timeline: Timeline,
        time: i64,
    ) {
        let chunk = Arc::new(
            Chunk::builder(entity.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(timeline, time)]),
                    (
                        MyPoints::descriptor_points(),
                        &[MyPoint::new(1.0, 1.0)] as &dyn ComponentBatch,
                    ),
                )
                .build()
                .unwrap(),
        );
        let events = store.insert_chunk(&chunk).unwrap();
        meta.on_events(manifest, store, &events);
    }

    #[test]
    fn row_count_tracks_additions() {
        let mut store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let mut meta = DataMetaPerTimeline::default();
        let manifest = RrdManifestIndex::default();

        let entity: EntityPath = "entity".into();
        let tl_frame = Timeline::new_sequence("frame");
        let tl_log = Timeline::new_sequence("log_time");

        // Unknown timeline returns 0.
        assert_eq!(
            meta.row_count_for_timeline(&TimelineName::from("missing")),
            0
        );

        // Insert three rows on tl_frame.
        for t in [10, 20, 30] {
            insert_and_update(&mut store, &mut meta, &manifest, &entity, tl_frame, t);
        }
        assert_eq!(meta.row_count_for_timeline(tl_frame.name()), 3);

        // Different timeline is still 0.
        assert_eq!(meta.row_count_for_timeline(tl_log.name()), 0);

        // Insert on a second timeline.
        insert_and_update(&mut store, &mut meta, &manifest, &entity, tl_log, 100);
        assert_eq!(meta.row_count_for_timeline(tl_log.name()), 1);
        // First timeline unchanged.
        assert_eq!(meta.row_count_for_timeline(tl_frame.name()), 3);
    }

    #[test]
    fn row_count_tracks_deletions() {
        let mut store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let mut meta = DataMetaPerTimeline::default();
        let manifest = RrdManifestIndex::default();

        let entity: EntityPath = "entity".into();
        let tl = Timeline::new_sequence("frame");

        for t in [10, 20, 30] {
            insert_and_update(&mut store, &mut meta, &manifest, &entity, tl, t);
        }
        assert_eq!(meta.row_count_for_timeline(tl.name()), 3);

        // GC everything and feed deletion events.
        let (gc_events, _stats) =
            store.gc(&re_chunk_store::GarbageCollectionOptions::gc_everything());
        meta.on_events(&manifest, &store, &gc_events);

        assert_eq!(meta.row_count_for_timeline(tl.name()), 0);
    }

    #[test]
    fn multiple_entities_contribute_to_same_timeline() {
        let mut store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let mut meta = DataMetaPerTimeline::default();
        let manifest = RrdManifestIndex::default();

        let tl = Timeline::new_sequence("frame");
        let entity_a: EntityPath = "a".into();
        let entity_b: EntityPath = "b".into();

        insert_and_update(&mut store, &mut meta, &manifest, &entity_a, tl, 10);
        insert_and_update(&mut store, &mut meta, &manifest, &entity_b, tl, 20);

        // Row count sums across entities.
        assert_eq!(meta.row_count_for_timeline(tl.name()), 2);
    }

    #[test]
    fn multi_row_chunk_counted_correctly() {
        let mut store = ChunkStore::new(
            StoreId::random(re_log_types::StoreKind::Recording, "test"),
            ChunkStoreConfig::ALL_DISABLED,
        );
        let mut meta = DataMetaPerTimeline::default();
        let manifest = RrdManifestIndex::default();

        let entity: EntityPath = "entity".into();
        let tl = Timeline::new_sequence("frame");
        let point = MyPoint::new(1.0, 1.0);

        let chunk = Arc::new(
            Chunk::builder(entity.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(tl, 10)]),
                    (
                        MyPoints::descriptor_points(),
                        &[point] as &dyn ComponentBatch,
                    ),
                )
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(tl, 20)]),
                    (
                        MyPoints::descriptor_points(),
                        &[point] as &dyn ComponentBatch,
                    ),
                )
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(tl, 30)]),
                    (
                        MyPoints::descriptor_points(),
                        &[point] as &dyn ComponentBatch,
                    ),
                )
                .build()
                .unwrap(),
        );
        let events = store.insert_chunk(&chunk).unwrap();
        meta.on_events(&manifest, &store, &events);

        assert_eq!(meta.row_count_for_timeline(tl.name()), 3);
    }

    /// Build chunks at given times, create an RRD manifest from them, and return both.
    fn build_manifest_chunks(
        entity: &EntityPath,
        timeline: Timeline,
        times: &[i64],
        store_id: &StoreId,
    ) -> (Vec<Arc<Chunk>>, Arc<re_log_encoding::RrdManifest>) {
        let point = MyPoint::new(1.0, 1.0);
        let chunks: Vec<Arc<Chunk>> = times
            .iter()
            .map(|&t| {
                Arc::new(
                    Chunk::builder(entity.clone())
                        .with_component_batch(
                            RowId::new(),
                            TimePoint::from_iter([(timeline, t)]),
                            (
                                MyPoints::descriptor_points(),
                                &[point] as &dyn ComponentBatch,
                            ),
                        )
                        .build()
                        .unwrap(),
                )
            })
            .collect();

        let manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            store_id.clone(),
            chunks.iter().map(|c| &**c),
        )
        .unwrap();

        (chunks, manifest)
    }

    #[test]
    fn virtual_addition_row_count() {
        let store_id = StoreId::random(re_log_types::StoreKind::Recording, "test");
        let mut store = ChunkStore::new(store_id.clone(), ChunkStoreConfig::ALL_DISABLED);
        let mut meta = DataMetaPerTimeline::default();
        let mut manifest_index = RrdManifestIndex::default();

        let entity: EntityPath = "entity".into();
        let tl = Timeline::new_sequence("frame");

        let (_, rrd_manifest) = build_manifest_chunks(&entity, tl, &[10, 20, 30], &store_id);

        // Insert the manifest virtually.
        let event = store.insert_rrd_manifest(rrd_manifest.clone()).unwrap();
        manifest_index.append(rrd_manifest);
        meta.on_events(&manifest_index, &store, &[event]);

        // Virtual rows should be counted.
        assert_eq!(meta.row_count_for_timeline(tl.name()), 3);
    }

    #[test]
    fn virtual_then_physical_no_double_count() {
        let store_id = StoreId::random(re_log_types::StoreKind::Recording, "test");
        let mut store = ChunkStore::new(store_id.clone(), ChunkStoreConfig::ALL_DISABLED);
        let mut meta = DataMetaPerTimeline::default();
        let mut manifest_index = RrdManifestIndex::default();

        let entity: EntityPath = "entity".into();
        let tl = Timeline::new_sequence("frame");

        let (chunks, rrd_manifest) = build_manifest_chunks(&entity, tl, &[10, 20, 30], &store_id);

        // Load virtually first.
        let event = store.insert_rrd_manifest(rrd_manifest.clone()).unwrap();
        manifest_index.append(rrd_manifest);
        manifest_index.on_events(&store, std::slice::from_ref(&event));
        meta.on_events(&manifest_index, &store, std::slice::from_ref(&event));

        assert_eq!(meta.row_count_for_timeline(tl.name()), 3);

        // Now load the same chunks physically.
        for chunk in &chunks {
            let events = store.insert_chunk(chunk).unwrap();
            manifest_index.on_events(&store, &events);
            meta.on_events(&manifest_index, &store, &events);
        }

        // Physical additions for chunks that are in the manifest should not be double-counted.
        assert_eq!(meta.row_count_for_timeline(tl.name()), 3);
    }

    #[test]
    fn virtual_addition_multiple_entities() {
        let store_id = StoreId::random(re_log_types::StoreKind::Recording, "test");
        let mut store = ChunkStore::new(store_id.clone(), ChunkStoreConfig::ALL_DISABLED);
        let mut meta = DataMetaPerTimeline::default();
        let mut manifest_index = RrdManifestIndex::default();

        let tl = Timeline::new_sequence("frame");
        let entity_a: EntityPath = "a".into();
        let entity_b: EntityPath = "b".into();

        // Build one manifest that contains chunks for both entities.
        let point = MyPoint::new(1.0, 1.0);
        let chunks: Vec<Arc<Chunk>> = [
            (entity_a.clone(), 10),
            (entity_a.clone(), 20),
            (entity_b.clone(), 30),
            (entity_b.clone(), 40),
        ]
        .into_iter()
        .map(|(e, t)| {
            Arc::new(
                Chunk::builder(e)
                    .with_component_batch(
                        RowId::new(),
                        TimePoint::from_iter([(tl, t)]),
                        (
                            MyPoints::descriptor_points(),
                            &[point] as &dyn ComponentBatch,
                        ),
                    )
                    .build()
                    .unwrap(),
            )
        })
        .collect();

        let rrd_manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            store_id.clone(),
            chunks.iter().map(|c| &**c),
        )
        .unwrap();

        let event = store.insert_rrd_manifest(rrd_manifest.clone()).unwrap();
        manifest_index.append(rrd_manifest);
        meta.on_events(&manifest_index, &store, &[event]);

        assert_eq!(meta.row_count_for_timeline(tl.name()), 4);
    }
}
