use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use itertools::Itertools as _;
use re_chunk::{Chunk, ChunkId, ComponentIdentifier, RowId, TimelineName};
use re_log_encoding::{RrdManifest, RrdManifestTemporalMapEntry};

use crate::ChunkStore;

// ---

// TODO: should this track chunk IDs or actual chunks?
// -> i believe IDs make more sense here, and are consistent with the rest of the store.
//
// TODO: top-level docs & per-field docs
//
// TODO: i wonder if we should keep track of per-chunk stats... i think we will have to in order to
// maintain the store stats anyway, no?
//
// TODO: this is more of a `ChunkDirectLineage`.
#[derive(Debug, Clone)]
pub enum ChunkLineage {
    SplitFrom(ChunkId),
    CompactedFrom(BTreeSet<ChunkId>),
    ReferencedFrom(Arc<RrdManifest>),
    Volatile,
}

impl std::fmt::Display for ChunkLineage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SplitFrom(chunk_id) => f.write_fmt(format_args!("split-from:{chunk_id}")),

            Self::CompactedFrom(chunk_ids) => f.write_fmt(format_args!(
                "compacted-from:[{}]",
                chunk_ids.iter().join(", ")
            )),

            // TODO: we dont have any nice Display for a storeid? really?
            Self::ReferencedFrom(rrd_manifest) => f.write_fmt(format_args!(
                "origin:RrdManifest#{:?}",
                rrd_manifest.store_id
            )),

            Self::Volatile => f.write_str("origin:<volatile> (cannot be re-fetched)"),
        }
    }
}

// TODO: I guess that, in practice, almost nobody will actually use these things, besides the store itself.
// -> i.e. query results should automatically resolve top-level roots by default, right?
impl ChunkStore {
    // TODO: find_origins or something, i guess?

    // TODO: docs
    pub fn format_lineage(&self, chunk_id: &ChunkId) -> String {
        // TODO: obviously this is insane
        fn is_chunk_static(store: &ChunkStore, chunk_id: &ChunkId) -> bool {
            if store
                .chunks_per_chunk_id
                .get(chunk_id)
                .map_or(false, |c| c.is_static())
            {
                return true;
            }

            // TODO: need to investigate all possible relevant manifests???
            for (_, rrd_manifest) in store.find_lineage_roots(chunk_id) {
                for (id, is_static) in itertools::izip!(
                    rrd_manifest.col_chunk_id().unwrap(),
                    rrd_manifest.col_chunk_is_static().unwrap()
                ) {
                    if *chunk_id == id {
                        return is_static;
                    }
                }
            }

            return false;
        }

        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, depth: usize) -> String {
            let lineage = store.chunks_lineage.get(chunk_id);

            let chunk = store.chunks_per_chunk_id.get(chunk_id);
            let status = if chunk.is_some() {
                "loaded"
            } else {
                if store.find_lineage_roots(chunk_id).is_empty() {
                    "lost"
                } else {
                    "offloaded"
                }
            };

            // TODO: that is very much something we should be able to know before loading tho
            let is_static = if is_chunk_static(store, chunk_id) {
                " <static>"
            } else {
                ""
            };

            let indent: String = std::iter::repeat_n(' ', (depth + 1) * 4).collect(); // TODO: silly
            format!("{chunk_id} (status:{status}{is_static})\n")
                + &match lineage {
                    Some(ChunkLineage::SplitFrom(chunk_id)) => {
                        format!(
                            "{indent}split-from: {}",
                            recurse(store, chunk_id, depth + 1)
                        )
                    }

                    Some(ChunkLineage::CompactedFrom(chunk_ids)) => chunk_ids
                        .iter()
                        .map(|chunk_id| {
                            format!(
                                "{indent}compacted-from: {}",
                                recurse(store, chunk_id, depth + 1)
                            )
                        })
                        .join("\n"),

                    Some(lineage) => {
                        format!("{indent}{lineage}")
                    }

                    // TODO: never supposed to happen then
                    None => format!("{indent}<invalid>"),
                }
        }

        recurse(self, chunk_id, 0)
    }

    /// Returns the top-level persistent roots of a given chunk.
    //
    // TODO: explain wtf that means.
    pub fn find_lineage_roots(&self, chunk_id: &ChunkId) -> Vec<(ChunkId, Arc<RrdManifest>)> {
        fn recurse(
            store: &ChunkStore,
            chunk_id: &ChunkId,
            roots: &mut Vec<(ChunkId, Arc<RrdManifest>)>,
        ) {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkLineage::SplitFrom(chunk_id)) => recurse(store, chunk_id, roots),

                Some(ChunkLineage::CompactedFrom(chunk_ids)) => {
                    for chunk_id in chunk_ids {
                        recurse(store, chunk_id, roots);
                    }
                }

                Some(ChunkLineage::ReferencedFrom(rrd_manifest)) => {
                    roots.push((*chunk_id, rrd_manifest.clone()));
                }

                _ => {}
            }
        }

        let mut roots = Vec::new();
        recurse(self, chunk_id, &mut roots);

        roots
    }
}

// TODO: tests:
// * compaction
// * splitting
// * ReferencedFrom... once we can do that
// * gc shouldnt affect anything
// * static behavior too, i guess

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use re_chunk::{EntityPath, Timeline};
    use re_log_types::StoreId;
    use re_log_types::example_components::{MyColor, MyLabel, MyPoint, MyPoints};
    use re_log_types::external::re_tuid::Tuid;

    use crate::{ChunkStoreConfig, GarbageCollectionOptions};

    use super::*;

    #[test]
    fn lineage_basics() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            dbg!(ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 3,
                chunk_max_rows_if_unsorted: 3,
            }),
        );

        let mut next_chunk_id = next_chunk_id_generator(1);

        let entity_path = EntityPath::from("this/that");
        let timepoint = [(Timeline::new_sequence("frame"), 1)];
        let points = &[MyPoint::new(1.0, 1.0)];

        let mut build_chunk = |num_rows: usize| {
            let mut builder = Chunk::builder_with_id(next_chunk_id(), entity_path.clone());

            for _ in 0..num_rows {
                builder = builder.with_component_batches(
                    RowId::new(),
                    timepoint,
                    [(MyPoints::descriptor_points(), points as _)],
                );
            }

            Arc::new(builder.build().unwrap())
        };

        store.insert_chunk(&build_chunk(1)).unwrap();
        store.insert_chunk(&build_chunk(1)).unwrap();
        store.insert_chunk(&build_chunk(1)).unwrap();
        store.insert_chunk(&build_chunk(1)).unwrap();
        store.insert_chunk(&build_chunk(3)).unwrap();
        store.insert_chunk(&build_chunk(3)).unwrap();
        store.insert_chunk(&build_chunk(6)).unwrap();

        // TODO: isn't status:lost the same thing as volatile? why do we show both?
        let dump_lineage_report = |store: &ChunkStore| {
            let mut lineage_report = Vec::new();
            eprintln!("---");
            eprintln!("ℹ️ There are 3 possible statuses for a Chunk:");
            eprintln!(
                "* `loaded`: the chunk is loaded into the ChunkStore and therefore available in local memory"
            );
            eprintln!(
                "* `offloaded`: the chunk has been dropped from local memory but can be re-fetched from its original source"
            );
            eprintln!(
                "* `lost`: the chunk has been dropped from local memory and cannot be re-fetched (no persistent source: it's volatile)"
            );
            eprintln!("---");

            // TODO: i guess we care about 2 kinds of starting points:
            // * loaded chunks, because of course we do
            // * chunks inserted via a manifest, loaded or not, because we need to track that visually
            let starting_chunk_ids: BTreeSet<ChunkId> = store
                .chunks_per_chunk_id
                .keys()
                .copied()
                .chain(store.chunks_lineage.values().flat_map(|lineage| {
                    if let ChunkLineage::ReferencedFrom(rrd_manifest) = lineage {
                        rrd_manifest.col_chunk_id().unwrap().collect_vec()
                    } else {
                        vec![]
                    }
                }))
                .collect();

            let starting_chunk_ids = store.chunks_per_chunk_id.keys().copied();
            for chunk_id in starting_chunk_ids {
                lineage_report.push(store.format_lineage(&chunk_id));
            }

            eprintln!("{}", lineage_report.join("\n---\n"));
        };

        eprintln!("\n\nLineage report (only the volatile chunks)");
        dump_lineage_report(&store);

        let remote_store =
            create_nasty_recording(42, "my_nasty_segment", &["my_nasty_entity"]).unwrap();
        let remote_rrd_manifest = Arc::new(remote_store.to_rrd_manifest().unwrap());
        store.insert_rrd_manifest(remote_rrd_manifest).unwrap();

        eprintln!("\n\nLineage report (inserted an RRD manifest, but fully unloaded)");
        dump_lineage_report(&store);

        // Simulate a full lazy load
        for chunk in remote_store.iter_chunks() {
            store.insert_chunk(chunk).unwrap();
        }

        eprintln!("\n\nLineage report (RRD manifest contents fully loaded)");
        dump_lineage_report(&store);

        store.gc(&GarbageCollectionOptions::gc_everything());

        eprintln!("\n\nLineage report (GC'd everything)");
        dump_lineage_report(&store);
    }

    fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
        let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
        move || {
            chunk_id = chunk_id.next();
            chunk_id
        }
    }

    fn next_row_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::RowId {
        let mut row_id = re_chunk::RowId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
        move || {
            row_id = row_id.next();
            row_id
        }
    }

    // ---

    // TODO: surely we're not copying that freaking thing _again_

    /// Indicates the prefix used for all `Tuid`s in a given recording, i.e.
    /// ```ignore
    /// Tuid::from_nanos_and_inc(TuidPrefix, 0)
    /// ```
    pub type TuidPrefix = u64;

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
        use re_log_types::{EntityPath, TimeInt, TimeType, build_frame_nr, build_log_time};

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
}
