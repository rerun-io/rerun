use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use arrow::datatypes::DataType as ArrowDataType;
use nohash_hasher::IntMap;
use re_arrow_util::RecordBatchTestExt;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, TimelineName};
use re_log_encoding::{RrdManifest, RrdManifestTemporalMapEntry};
use re_log_types::{EntityPath, StoreId, TimeInt, TimeType};
use re_types_core::{ComponentDescriptor, ComponentType};

use crate::store::ChunkIdSetPerTime;
use crate::{
    ChunkLineage, ChunkStore, ChunkStoreChunkStats, ChunkStoreConfig, ChunkStoreError,
    ChunkStoreResult,
};

// ---

// TODO: step 1:
// * build a store from an RrdManifest (compaction force disabled for now)
// * should be able to dumb the store as a manifest and get the same manifest as the one used to load

impl ChunkStore {
    // TODO: ye im not aiming for quality nor performance here
    //
    // TODO: we should probably not allow insert_chunk() on that thing...
    // TODO: can this even fail?

    // TODO: why are virtual stores not the default then?

    // TODO: I guess we could just exempt clears too? or will clears just resolve themselves by
    // virtue of re_query asking for them JIT?

    /// This instantiates a new *virtual* [`ChunkStore`] from an [`RrdManifest`].
    //
    // TODO: explain wtf that means for end-users.
    // TODO: explain what that means internally (e.g.: `chunks_per_chunk_id` is left empty, and it all works from there).
    pub fn from_rrd_manifest(rrd_manifest: Arc<RrdManifest>) -> anyhow::Result<Self> {
        let mut store = Self::new(
            rrd_manifest.store_id.clone(),
            // TODO: we must disable chunk compaction and splitting until we perform proper lineage
            // tracking, else we cannot communicate which chunks are missing in a reliable fashion.
            ChunkStoreConfig::COMPACTION_DISABLED,
        );

        // TODO: okay well, let's see what's the minimum we can get away with i guess
        let Self {
            id: _,
            config: _,
            time_type_registry,
            type_registry,
            per_column_metadata,
            chunks_per_chunk_id: _, // TODO: by definition, we never fill that one!
            chunks_lineage,
            chunk_ids_per_min_row_id: _, // TODO: what do we do with this one, remind me?
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_chunks_stats: _, // TODO: and we're lacking some info in footers
            static_chunk_ids_per_entity,
            static_chunks_stats: _, // TODO: and we're lacking some info in footers
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = &mut store;

        // TODO: well we need a col_arbitrary_component thing?

        *static_chunk_ids_per_entity = rrd_manifest.get_static_data_as_a_map()?;

        let xxx = rrd_manifest.get_temporal_data_as_a_map()?;

        // TODO: just return flat vecs rather than this mess.
        for (entity_path, per_timeline) in xxx {
            for (timeline, per_component) in per_timeline {
                for (component, per_chunk) in per_component {
                    for (chunk_id, entry) in per_chunk {
                        let RrdManifestTemporalMapEntry {
                            time_range,
                            num_rows: _,
                        } = entry;

                        chunks_lineage
                            .insert(chunk_id, ChunkLineage::ReferencedFrom(rrd_manifest.clone()));

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

        Ok(store)
    }

    // TODO: this is not expected to ever trigger any events, right?
    // TODO: aka `insert_virtual_chunks`?
    // TODO: not anyhow
    pub fn insert_rrd_manifest(&mut self, rrd_manifest: Arc<RrdManifest>) -> anyhow::Result<()> {
        // TODO: okay well, let's see what's the minimum we can get away with i guess
        // TODO: i dont think stats should include virtual stuff anyhow
        // TODO: we really a better way to track what is physical and what isn't
        // -> although i guess everything that exists right now is by definition physical
        //
        // So... yeah, do we actually care about filling up these things in the end?
        let Self {
            id: _,
            config: _,
            time_type_registry,
            type_registry,
            per_column_metadata,
            chunks_per_chunk_id: _, // TODO: by definition, we never fill that one!
            chunks_lineage,
            chunk_ids_per_min_row_id: _, // TODO: what do we do with this one, remind me?
            temporal_chunk_ids_per_entity_per_component,
            temporal_chunk_ids_per_entity,
            temporal_chunks_stats: _, // TODO: and we're lacking some info in footers
            static_chunk_ids_per_entity,
            static_chunks_stats: _, // TODO: and we're lacking some info in footers
            insert_id: _,
            gc_id: _,
            event_id: _,
        } = self;

        // TODO: well we need a col_arbitrary_component thing?

        // TODO: also we really need to check whether this chunks already exist, as we usually do.

        *static_chunk_ids_per_entity = rrd_manifest.get_static_data_as_a_map()?;

        let xxx = rrd_manifest.get_temporal_data_as_a_map()?;

        // TODO: just return flat vecs rather than this mess.
        for (entity_path, per_timeline) in xxx {
            for (timeline, per_component) in per_timeline {
                for (component, per_chunk) in per_component {
                    for (chunk_id, entry) in per_chunk {
                        let RrdManifestTemporalMapEntry {
                            time_range,
                            num_rows: _,
                        } = entry;

                        chunks_lineage
                            .insert(chunk_id, ChunkLineage::ReferencedFrom(rrd_manifest.clone()));

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

        Ok(())
    }

    // TODO: im now fairly convinced that this is a terrible idea.
    // -> unless we explicitly document that this acts in the silliest way possible:
    //    * only the physical chunks, in whatever state they are (i.e. at the bottom of the lineage tree)
    //
    // TODO: there are 2 situations where you want this to work:
    // * `self` is a standard store
    // * `self` is a pure virtual store
    // -> or 3, mix of both?
    //
    // TODO: we should be calling this from the re_server impl, is my guess
    // -> which might require an arbitrary callback to feed extra per-row data, possibly
    pub fn to_rrd_manifest(&self) -> re_log_encoding::CodecResult<RrdManifest> {
        let mut rrd_manifest_builder = re_log_encoding::RrdManifestBuilder::default();

        let mut offset = 0;
        for chunk in self.iter_chunks() {
            let chunk_batch = chunk.to_chunk_batch()?;

            // Not a totally accurate value, but we're certainly not going to encode every chunk
            // into IPC bytes just to figure out their uncompressed size either.
            //
            // This is fine for 2 reasons:
            // 1. The reported size is mostly for human and automated heuristics (e.g. "have I
            //    enough memory left to download this chunk?"), and so doesn't need to be exact.
            // 2. Reporting the size in terms of heap values is even better for such heuristics.
            use re_byte_size::SizeBytes as _;
            let byte_size_uncompressed = chunk.heap_size_bytes();

            // There is no such thing as "compressed data on disk" in the case of the OSS server,
            // since there's no disk to begin with. That's fine, we just re-use the
            // uncompressed values: the chunk-key (generated below) is what will be used to
            // accurately fetch the data in any case.
            //
            // TODO(cmc): we could also keep track of the compressed values originally fetched
            // from disk and/or network all the way into the OSS server's datastructures and
            // resurface them here but that doesn't seem to have any practical use, so not
            // worth the added complexity?
            let uncompressed_byte_span = re_span::Span {
                start: offset,
                len: byte_size_uncompressed,
            };

            offset += byte_size_uncompressed;

            rrd_manifest_builder.append(
                &chunk_batch,
                uncompressed_byte_span,
                byte_size_uncompressed,
            )?;

            // TODO: that's a good example: what should we do with the original chunk keys if the
            // thing was indeed virtual?
            // chunk_keys.push(
            //     crate::store::ChunkKey {
            //         chunk_id: chunk.id(),
            //         segment_id: segment_id.clone(),
            //         layer_name: layer_name.to_owned(),
            //         dataset_id: self.id(),
            //     }
            //     .encode()?,
            // );
        }

        let mut rrd_manifest = rrd_manifest_builder.build(self.id())?;
        rrd_manifest.sanity_check_cheap()?; // TODO
        rrd_manifest.sanity_check_heavy()?; // TODO

        Ok(rrd_manifest)
    }
}

// TODO: can't the chunkstore just keep a copy of the manifest that was used to initialize it?
// -> no, we ended up with an insert-rrd-manifest model rather than a from-rrd-manifest model

// TODO: but all in all: what do we want?
// * we want to be able to build a virtual store from a manifest
// * we want to be able to make parts of that store physical as time goes on and we load data
// * we want to track different levels of lineage:
//   * is a chunk the result of compacting other chunks?
//   * is a chunk the result of splitting another chunk?
//   * did a chunk came in via a manifest?
// * we want to be able to query a store, whether it's fully physical, or fully virtual, or a mix of both
//   * missing physical chunk IDs should be returned so the caller can load them
//     -> i.e. the store itself doesn't handle IO (that's up to the viewer, datafusion, etc)
//
// If we're heading towards a world where the physicality of a store is a spectrum rather than a
// boolean, then maybe it makes more sense to view `from_rrd_manifest` as `insert_from_manifest`
// instead. Then again, the former can be a stepping stone towards the latter.
//
// In such a world, it can feel normal to mix and match insertion of physical chunks vs. virtual chunks.
// This has important ramifications on GC: GCing a chunk that wasn't inserted via a manifest is
// irrecoverable. Only real-time logging should result in that kind of situation though, since even
// files should be loaded via manifests in the future (either explicitly, or via the OSS server), right?

#[test]
fn native_to_virtual_roundtrip() {
    let store_physical =
        create_nasty_recording(42, "my_nasty_segment", &["my_nasty_entity"]).unwrap();
    let rrd_manifest_physical = Arc::new(store_physical.to_rrd_manifest().unwrap());

    eprintln!("{}", rrd_manifest_physical.data.format_snapshot(true));

    // TODO: the problem is that a virtual store might not stay fully virtual for long, e.g.:
    // * new unrelated chunks might be added to it
    // * previous chunks might be compacted/splitted
    // * etc
    // -> i guess it's all the same lineage problem in the end, right?
    let store_virtual = ChunkStore::from_rrd_manifest(rrd_manifest_physical).unwrap();
}

// ---

// TODO: surely we're not copying that freaking thing _again_

use re_log_types::Timeline;
use re_log_types::external::re_tuid::Tuid;

/// Indicates the prefix used for all `Tuid`s in a given recording, i.e.
/// ```ignore
/// Tuid::from_nanos_and_inc(TuidPrefix, 0)
/// ```
pub type TuidPrefix = u64;

pub fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
    let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        chunk_id = chunk_id.next();
        chunk_id
    }
}

pub fn next_row_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::RowId {
    let mut row_id = re_chunk::RowId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
    move || {
        row_id = row_id.next();
        row_id
    }
}

/// Creates a very nasty recording with all kinds of partial updates, chunk overlaps, repeated
/// timestamps, duplicated chunks, partial multi-timelines, flat and recursive clears, etc.
///
/// This makes it a great recording to test things with for most situations.
fn create_nasty_recording(
    tuid_prefix: TuidPrefix,
    segment_id: &str,
    entity_paths: &[&str],
) -> anyhow::Result<ChunkStore> {
    use re_chunk::{Chunk, TimePoint};
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, build_frame_nr, build_log_time};

    let mut store = ChunkStore::new(
        StoreId::recording("some_app_id", segment_id),
        ChunkStoreConfig::COMPACTION_DISABLED, // TODO
    );

    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    let mut next_row_id = next_row_id_generator(tuid_prefix);

    /// So we can test duration-based indexes too.
    fn build_sim_time(t: impl TryInto<TimeInt>) -> (Timeline, TimeInt) {
        (
            Timeline::new("sim_time", TimeType::DurationNs),
            TimeInt::saturated_temporal(t),
        )
    }

    for entity_path in entity_paths {
        let entity_path = EntityPath::from(*entity_path);

        let frame1 = TimeInt::new_temporal(10);
        let frame2 = TimeInt::new_temporal(20);
        let frame3 = TimeInt::new_temporal(30);
        let frame4 = TimeInt::new_temporal(40);
        let frame5 = TimeInt::new_temporal(50);
        let frame6 = TimeInt::new_temporal(60);
        let frame7 = TimeInt::new_temporal(70);

        let points1 = MyPoint::from_iter(0..1);
        let points2 = MyPoint::from_iter(1..2);
        let points3 = MyPoint::from_iter(2..3);
        let points4 = MyPoint::from_iter(3..4);
        let points5 = MyPoint::from_iter(4..5);
        let points6 = MyPoint::from_iter(5..6);
        let points7_1 = MyPoint::from_iter(6..7);
        let points7_2 = MyPoint::from_iter(7..8);
        let points7_3 = MyPoint::from_iter(8..9);

        let colors3 = MyColor::from_iter(2..3);
        let colors4 = MyColor::from_iter(3..4);
        let colors5 = MyColor::from_iter(4..5);
        let colors7 = MyColor::from_iter(6..7);

        let labels1 = vec![MyLabel("a".to_owned())];
        let labels2 = vec![MyLabel("b".to_owned())];
        let labels3 = vec![MyLabel("c".to_owned())];

        let chunk1_1 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame1),
                    build_log_time(frame1.into()),
                    build_sim_time(frame1),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points1 as _)),
                    (MyPoints::descriptor_colors(), None),
                    (MyPoints::descriptor_labels(), Some(&labels1 as _)), // shadowed by static
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame3),
                    build_log_time(frame3.into()),
                    build_sim_time(frame3),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame5),
                    build_log_time(frame5.into()),
                    build_sim_time(frame5),
                ],
                [
                    (MyPoints::descriptor_points(), Some(&points5 as _)),
                    (MyPoints::descriptor_colors(), None),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_1 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [
                    build_frame_nr(frame7),
                    build_log_time(frame7.into()),
                    build_sim_time(frame7),
                ],
                [(MyPoints::descriptor_points(), Some(&points7_3 as _))],
            )
            .build()?;
        let chunk1_2 = chunk1_1.clone_as(next_chunk_id(), next_row_id());
        let chunk1_3 = chunk1_1.clone_as(next_chunk_id(), next_row_id());

        store.insert_chunk(&Arc::new(chunk1_1))?;
        store.insert_chunk(&Arc::new(chunk1_2))?; // x2!
        store.insert_chunk(&Arc::new(chunk1_3))?; // x3!

        let chunk2 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame3)],
                [
                    (MyPoints::descriptor_points(), Some(&points3 as _)),
                    (MyPoints::descriptor_colors(), Some(&colors3 as _)),
                ],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk2))?;

        let chunk3 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame2)],
                [(MyPoints::descriptor_points(), Some(&points2 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_points(), Some(&points4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame6)],
                [(MyPoints::descriptor_points(), Some(&points6 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk3))?;

        let chunk4 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame4)],
                [(MyPoints::descriptor_colors(), Some(&colors4 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame5)],
                [(MyPoints::descriptor_colors(), Some(&colors5 as _))],
            )
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame7)],
                [(MyPoints::descriptor_colors(), Some(&colors7 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk4))?;

        let chunk5 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels2 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk5))?;

        let chunk6 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                TimePoint::default(),
                [(MyPoints::descriptor_labels(), Some(&labels3 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk6))?;
    }

    for entity_path in entity_paths {
        let entity_path = EntityPath::from(*entity_path);

        let frame95 = TimeInt::new_temporal(950);
        let frame99 = TimeInt::new_temporal(990);

        let colors99 = MyColor::from_iter(99..100);

        let labels95 = vec![MyLabel("z".to_owned())];

        let chunk7 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame99)],
                [(MyPoints::descriptor_colors(), Some(&colors99 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk7))?;

        let chunk8 = Chunk::builder_with_id(next_chunk_id(), entity_path.clone())
            .with_sparse_component_batches(
                next_row_id(),
                [build_frame_nr(frame95)],
                [(MyPoints::descriptor_labels(), Some(&labels95 as _))],
            )
            .build()?;

        store.insert_chunk(&Arc::new(chunk8))?;
    }

    Ok(store)
}
