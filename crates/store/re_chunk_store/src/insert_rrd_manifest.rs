use std::sync::Arc;

use ahash::HashMap;

use re_log_encoding::{RrdManifest, RrdManifestTemporalMapEntry};

use crate::lineage::TrackedDirectChunkLineage;
use crate::store::ChunkIdSetPerTime;
use crate::{ChunkDirectLineage, ChunkId, ChunkStore, ChunkStoreDiff, ChunkStoreEvent};

// ---

/// The segment each chunk comes from, `None` for a manifest that does not name them.
fn segment_id_per_chunk(
    rrd_manifest: &RrdManifest,
) -> Option<HashMap<ChunkId, Arc<re_sdk_types::SegmentId>>> {
    let partition_ids = rrd_manifest.col_chunk_partition_id()?;

    // A manifest usually has many chunks referring to a small amount of segments (usually just one), so the
    // ids are shared.
    let mut per_partition_id: HashMap<&str, Arc<re_sdk_types::SegmentId>> = HashMap::default();

    Some(
        std::iter::zip(rrd_manifest.col_chunk_ids(), partition_ids.iter())
            .filter_map(|(chunk_id, partition_id)| {
                let partition_id = partition_id?;
                let segment_id = per_partition_id
                    .entry(partition_id)
                    .or_insert_with(|| Arc::new(re_sdk_types::SegmentId::from(partition_id)))
                    .clone();
                Some((*chunk_id, segment_id))
            })
            .collect(),
    )
}

impl ChunkStore {
    /// This insert a batch of virtual chunks into the store, according to the given [`RrdManifest`].
    ///
    /// All queries will return partial results until the missing physical data gets loaded in.
    #[must_use = "The chunk store events should be handled"]
    pub fn insert_rrd_manifest(&mut self, rrd_manifest: Arc<RrdManifest>) -> Vec<ChunkStoreEvent> {
        re_tracing::profile_function!();

        let Self {
            id: _,
            config: _,
            schema: _,                            // handled below
            physical_chunks_per_chunk_id: _,      // physical data only
            physical_chunk_ids_per_min_row_id: _, // physical data only
            chunks_lineage,
            dangling_splits: _, // cannot split during virtual insert
            split_on_ingest: _,
            leaky_compactions: _, // cannot compact during virtual insert
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_physical_chunks_stats: _, // stats are for physical data only
            static_chunk_ids_per_entity,
            static_chunks_stats: _, // stats are for physical data only
            queried_chunk_id_tracker: _,
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        // A manifest that does not name the segment of its chunks has them all in this one.
        let own_segment_id = Arc::new(re_sdk_types::SegmentId::from(
            rrd_manifest.store_id().recording_id(),
        ));
        let segment_id_per_chunk = segment_id_per_chunk(&rrd_manifest);
        let segment_id_of = |chunk_id: &ChunkId| {
            segment_id_per_chunk
                .as_ref()
                .and_then(|per_chunk| per_chunk.get(chunk_id))
                .unwrap_or(&own_segment_id)
                .clone()
        };

        let native_static_map = rrd_manifest.static_map();
        chunks_lineage.extend(
            native_static_map
                .values()
                .flat_map(|per_component| per_component.values())
                .map(|chunk_id| {
                    (
                        *chunk_id,
                        TrackedDirectChunkLineage {
                            lineage: ChunkDirectLineage::RootFromManifest {
                                is_static: true,
                                segment_id: segment_id_of(chunk_id),
                            },
                            ref_count: 0,
                            descends_from_manifest: true,
                        },
                    )
                }),
        );
        for (entity_path, per_component) in native_static_map {
            static_chunk_ids_per_entity
                .entry(entity_path.clone())
                .or_default()
                .extend(per_component.iter().map(|(&k, &v)| (k, v)));
        }

        let native_temporal_map = rrd_manifest.temporal_map();
        chunks_lineage.extend(
            native_temporal_map
                .values()
                .flat_map(|per_timeline| per_timeline.values())
                .flat_map(|per_component| per_component.values())
                .flat_map(|per_chunk| per_chunk.keys())
                .map(|chunk_id| {
                    (
                        *chunk_id,
                        TrackedDirectChunkLineage {
                            lineage: ChunkDirectLineage::RootFromManifest {
                                is_static: false,
                                segment_id: segment_id_of(chunk_id),
                            },
                            ref_count: 0,
                            descends_from_manifest: true,
                        },
                    )
                }),
        );
        for (entity_path, per_timeline) in native_temporal_map {
            for (timeline, per_component) in per_timeline {
                for (&component, per_chunk) in per_component {
                    for (&chunk_id, &entry) in per_chunk {
                        let RrdManifestTemporalMapEntry {
                            time_range,
                            num_rows: _,
                        } = entry;
                        // with component
                        {
                            let per_timeline = temporal_chunk_ids_per_entity_per_component
                                .entry(entity_path.clone())
                                .or_default();
                            let per_component = per_timeline.entry(*timeline.name()).or_default();
                            let ChunkIdSetPerTime {
                                max_interval_length,
                                per_start_time,
                                per_end_time,
                            } = per_component.entry(component).or_default();
                            *max_interval_length =
                                (*max_interval_length).max(time_range.abs_length());
                            per_start_time
                                .entry(time_range.min)
                                .or_default()
                                .insert(chunk_id);
                            per_end_time
                                .entry(time_range.max)
                                .or_default()
                                .insert(chunk_id);
                        }

                        // without component
                        {
                            let per_timeline = temporal_chunk_ids_per_entity
                                .entry(entity_path.clone())
                                .or_default();
                            let ChunkIdSetPerTime {
                                max_interval_length,
                                per_start_time,
                                per_end_time,
                            } = per_timeline.entry(*timeline.name()).or_default();
                            *max_interval_length =
                                (*max_interval_length).max(time_range.abs_length());
                            per_start_time
                                .entry(time_range.min)
                                .or_default()
                                .insert(chunk_id);
                            per_end_time
                                .entry(time_range.max)
                                .or_default()
                                .insert(chunk_id);
                        }
                    }
                }
            }
        }

        let event = ChunkStoreEvent {
            store_id: self.id.clone(),
            store_generation: self.generation(),
            event_id: self
                .event_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            diff: ChunkStoreDiff::virtual_addition(rrd_manifest),
        };

        let new_columns = self.schema.on_events(std::slice::from_ref(&event));

        let mut events = vec![event];

        if !new_columns.is_empty() {
            events.push(ChunkStoreEvent {
                store_id: self.id.clone(),
                store_generation: self.generation(),
                event_id: self
                    .event_id
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                diff: ChunkStoreDiff::SchemaAddition(crate::ChunkStoreDiffSchemaAddition {
                    new_columns,
                }),
            });
        }

        if self.config.enable_changelog {
            Self::on_events(&events);
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::{Chunk, EntityPath, RowId, TimePoint, Timeline};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use similar_asserts::assert_eq;

    use super::*;

    /// `insert_rrd_manifest` should emit a `SchemaAddition` with the manifest's columns.
    #[test]
    fn schema_addition_from_manifest() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store_id =
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app");
        let mut store = ChunkStore::new(store_id.clone(), Default::default());

        let entity_path = EntityPath::from("this/that");
        let tl = Timeline::new_sequence("frame");
        let point = MyPoint::new(1.0, 1.0);

        let chunks: Vec<Arc<Chunk>> = [10, 20]
            .into_iter()
            .map(|t| {
                Arc::new(
                    Chunk::builder(entity_path.clone())
                        .with_component_batch(
                            RowId::new(),
                            TimePoint::from_iter([(tl, t)]),
                            (MyPoints::descriptor_points(), &[point] as _),
                        )
                        .build()
                        .unwrap(),
                )
            })
            .collect();

        let rrd_manifest = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            store_id,
            chunks.iter().map(|c| &**c),
        )?;

        let events = store.insert_rrd_manifest(rrd_manifest);
        assert_eq!(events.len(), 2);
        assert!(events[0].is_virtual_addition());
        let schema_add = match &events[1].diff {
            ChunkStoreDiff::SchemaAddition(sa) => sa,
            other => panic!("expected SchemaAddition, got {other:?}"),
        };
        assert_eq!(schema_add.new_columns.len(), 1);
        assert_eq!(schema_add.new_columns[0].entity_path, entity_path);
        assert!(!schema_add.new_columns[0].components.is_empty());

        // Inserting the same manifest again should NOT emit a second SchemaAddition.
        let rrd_manifest2 = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
            chunks.iter().map(|c| &**c),
        )?;
        let events2 = store.insert_rrd_manifest(rrd_manifest2);
        assert!(
            !events2.iter().any(|e| e.is_schema_addition()),
            "re-inserting a manifest with the same columns should not emit SchemaAddition"
        );

        Ok(())
    }

    /// Manifest with temporal data followed by manifest with static data:
    /// `is_static` should transition and re-emit a `SchemaAddition`.
    #[test]
    fn schema_static_transition_from_manifest() -> anyhow::Result<()> {
        re_log::setup_logging();

        let store_id =
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app");
        let mut store = ChunkStore::new(store_id.clone(), Default::default());

        let entity_path = EntityPath::from("this/that");
        let tl = Timeline::new_sequence("frame");
        let point = MyPoint::new(1.0, 1.0);

        // First manifest: temporal-only data.
        let temporal_chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::from_iter([(tl, 10)]),
                    (MyPoints::descriptor_points(), &[point] as _),
                )
                .build()?,
        );
        let manifest_temporal = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            store_id.clone(),
            std::iter::once(&*temporal_chunk),
        )?;

        let events = store.insert_rrd_manifest(manifest_temporal);
        assert_eq!(events.len(), 2);
        assert!(events[0].is_virtual_addition());
        let schema_add = match &events[1].diff {
            ChunkStoreDiff::SchemaAddition(sa) => sa,
            other => panic!("expected SchemaAddition, got {other:?}"),
        };
        assert!(
            !schema_add.new_columns[0].components[0].is_static,
            "first manifest is temporal-only"
        );

        // Second manifest: same component but with static data.
        let static_chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batch(
                    RowId::new(),
                    TimePoint::STATIC,
                    (MyPoints::descriptor_points(), &[point] as _),
                )
                .build()?,
        );
        let manifest_static = re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            store_id,
            std::iter::once(&*static_chunk),
        )?;

        let events = store.insert_rrd_manifest(manifest_static);
        assert_eq!(events.len(), 2);
        assert!(events[0].is_virtual_addition());
        let schema_add = match &events[1].diff {
            ChunkStoreDiff::SchemaAddition(sa) => sa,
            other => panic!("expected SchemaAddition for is_static transition, got {other:?}"),
        };
        assert!(
            schema_add.new_columns[0].components[0].is_static,
            "component should now be is_static after static manifest"
        );

        Ok(())
    }

    /// A temporal and a static chunk on the same entity, so both lineage paths of
    /// `insert_rrd_manifest` are covered.
    fn temporal_and_static_chunks(
        entity_path: &EntityPath,
        timepoint: TimePoint,
    ) -> anyhow::Result<Vec<Arc<Chunk>>> {
        let point = MyPoint::new(1.0, 1.0);

        Ok(vec![
            Arc::new(
                Chunk::builder(entity_path.clone())
                    .with_component_batch(
                        RowId::new(),
                        timepoint,
                        (MyPoints::descriptor_points(), &[point] as _),
                    )
                    .build()?,
            ),
            Arc::new(
                Chunk::builder(entity_path.clone())
                    .with_component_batch(
                        RowId::new(),
                        TimePoint::STATIC,
                        (MyPoints::descriptor_points(), &[point] as _),
                    )
                    .build()?,
            ),
        ])
    }

    /// A manifest describing the store's own segment carries the same recording id as the store,
    /// while a manifest for another segment describes an asset that was pulled into this store.
    /// Inserting each on its own names the segment of every virtual chunk in its lineage, and
    /// covers the entities and components of both segments.
    #[test]
    fn manifest_marks_lineage_with_its_source_segment() -> anyhow::Result<()> {
        re_log::setup_logging();

        let own_segment = "segment_of_this_store";
        let asset_segment = "segment_of_the_asset";

        let store_id = re_log_types::StoreId::new(
            re_log_types::StoreKind::Recording,
            "dataset_entry_id",
            own_segment,
        );
        let mut store = ChunkStore::new(store_id.clone(), Default::default());

        let own_entity_path = EntityPath::from("this/that");
        let asset_entity_path = EntityPath::from("the/asset");
        let tl = Timeline::new_sequence("frame");

        let own_chunks =
            temporal_and_static_chunks(&own_entity_path, TimePoint::from_iter([(tl, 10)]))?;
        let asset_chunks =
            temporal_and_static_chunks(&asset_entity_path, TimePoint::from_iter([(tl, 20)]))?;

        // Both manifests were written by the recording SDK, so they name the logging application
        // rather than the dataset the store was opened from.
        let own_manifest_store_id = re_log_types::StoreId::new(
            re_log_types::StoreKind::Recording,
            "recorded_app",
            own_segment,
        );
        let asset_store_id = re_log_types::StoreId::new(
            re_log_types::StoreKind::Recording,
            "recorded_app",
            asset_segment,
        );
        let expected_own_segment =
            re_sdk_types::SegmentId::from(own_manifest_store_id.recording_id());
        let expected_asset_segment = re_sdk_types::SegmentId::from(asset_store_id.recording_id());

        _ = store.insert_rrd_manifest(re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            own_manifest_store_id,
            own_chunks.iter().map(|chunk| &**chunk),
        )?);
        _ = store.insert_rrd_manifest(re_log_encoding::RrdManifest::build_in_memory_from_chunks(
            asset_store_id,
            asset_chunks.iter().map(|chunk| &**chunk),
        )?);

        let segment_of = |chunk: &Chunk| match &store
            .chunks_lineage
            .get(&chunk.id())
            .expect("every chunk in a manifest gets a lineage")
            .lineage
        {
            ChunkDirectLineage::RootFromManifest { segment_id, .. } => (**segment_id).clone(),
            other => panic!("expected a manifest root, got {other:?}"),
        };

        for chunk in &own_chunks {
            assert_eq!(segment_of(chunk), expected_own_segment);
            assert_eq!(
                store.find_source_segments(&chunk.id()),
                std::iter::once(expected_own_segment.clone()).collect(),
                "a manifest for the store's own segment names that segment"
            );
        }
        for chunk in &asset_chunks {
            assert_eq!(segment_of(chunk), expected_asset_segment);
            assert_eq!(
                store.find_source_segments(&chunk.id()),
                std::iter::once(expected_asset_segment.clone()).collect(),
                "a manifest for another segment is an asset, named after that segment"
            );
        }

        for entity_path in [&own_entity_path, &asset_entity_path] {
            assert!(
                store.entity_tree().subtree(entity_path).is_some(),
                "the entity tree covers every segment, missing: {entity_path}"
            );
            assert!(
                store
                    .schema()
                    .all_components_for_entity(entity_path)
                    .is_some_and(|components| !components.is_empty()),
                "the schema covers the components of every segment, missing: {entity_path}"
            );
        }

        Ok(())
    }

    /// A manifest of the given chunks as a server serves it, with a `chunk_partition_id` column
    /// naming the segment they came from.
    fn served_manifest(
        segment: &str,
        chunks: &[Arc<Chunk>],
    ) -> anyhow::Result<re_log_encoding::RrdManifest> {
        // The manifest was written by the recording SDK, so it names the logging application rather
        // than the dataset the store was opened from.
        let store_id =
            re_log_types::StoreId::new(re_log_types::StoreKind::Recording, "recorded_app", segment);
        let raw = re_log_encoding::RawRrdManifest::build_in_memory_from_chunks(
            store_id,
            chunks.iter().map(|chunk| &**chunk),
        )?;

        let (schema, mut columns, row_count) = raw.data.clone().into_parts();
        let mut fields = schema.fields.to_vec();
        fields.push(Arc::new(
            re_log_encoding::HubRrdManifest::field_chunk_partition_id(),
        ));
        columns.push(Arc::new(arrow::array::StringArray::from_iter_values(
            std::iter::repeat_n(segment, row_count),
        )));

        let data = arrow::array::RecordBatch::try_new_with_options(
            Arc::new(arrow::datatypes::Schema::new_with_metadata(
                fields,
                schema.metadata.clone(),
            )),
            columns,
            &arrow::array::RecordBatchOptions::new().with_row_count(Some(row_count)),
        )?;

        Ok(re_log_encoding::RrdManifest::try_new(
            &re_log_encoding::RawRrdManifest { data, ..raw },
        )?)
    }

    /// A manifest served by a server names the segment of every chunk, so one manifest can describe
    /// the chunks of several segments. Inserting such a concatenation names each chunk's own segment
    /// in its lineage, and covers the entities and components of every part.
    #[test]
    fn concatenated_manifest_names_the_segment_of_every_chunk() -> anyhow::Result<()> {
        re_log::setup_logging();

        let own_segment = "segment_of_this_store";
        let asset_segment = "segment_of_the_asset";

        let mut store = ChunkStore::new(
            re_log_types::StoreId::new(
                re_log_types::StoreKind::Recording,
                "dataset_entry_id",
                own_segment,
            ),
            Default::default(),
        );

        let own_entity_path = EntityPath::from("this/that");
        let asset_entity_path = EntityPath::from("the/asset");
        let tl = Timeline::new_sequence("frame");

        let own_chunks =
            temporal_and_static_chunks(&own_entity_path, TimePoint::from_iter([(tl, 10)]))?;
        let asset_chunks =
            temporal_and_static_chunks(&asset_entity_path, TimePoint::from_iter([(tl, 20)]))?;

        let own_manifest = served_manifest(own_segment, &own_chunks)?;
        let asset_manifest = served_manifest(asset_segment, &asset_chunks)?;

        _ = store.insert_rrd_manifest(Arc::new(re_log_encoding::RrdManifest::merge(&[
            &own_manifest,
            &asset_manifest,
        ])?));

        let segment_of = |chunk: &Chunk| match &store
            .chunks_lineage
            .get(&chunk.id())
            .expect("every chunk in a manifest gets a lineage")
            .lineage
        {
            ChunkDirectLineage::RootFromManifest { segment_id, .. } => (**segment_id).clone(),
            other => panic!("expected a manifest root, got {other:?}"),
        };

        for (chunks, segment) in [(&own_chunks, own_segment), (&asset_chunks, asset_segment)] {
            let expected = re_sdk_types::SegmentId::from(segment);
            for chunk in chunks {
                assert_eq!(segment_of(chunk), expected);
                assert_eq!(
                    store.find_source_segments(&chunk.id()),
                    std::iter::once(expected.clone()).collect(),
                    "a chunk is named after the segment its part of the manifest came from"
                );
            }
        }

        for entity_path in [&own_entity_path, &asset_entity_path] {
            assert!(
                store.entity_tree().subtree(entity_path).is_some(),
                "the entity tree covers every part, missing: {entity_path}"
            );
            assert!(
                store
                    .schema()
                    .all_components_for_entity(entity_path)
                    .is_some_and(|components| !components.is_empty()),
                "the schema covers the components of every part, missing: {entity_path}"
            );
        }

        Ok(())
    }
}
