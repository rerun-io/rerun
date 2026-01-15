use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use itertools::Itertools as _;

use re_chunk::{Chunk, ChunkId};
use re_log_encoding::RrdManifest;

use crate::ChunkStore;

// ---

/// How a chunk relates its direct ancestor(s).
///
/// These ancestors can be other chunk(s) or, at the top of the lineage tree, the origin of where
/// the data came from in the first place (volatile memory vs. an RRD manifest).
///
/// This type never holds any kind of strong reference towards [`Chunk`]s.
/// This makes it usable in virtual contexts where lineage information alone should never force the
/// underlying data to remain in local memory, such as the store's virtual indexes.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkDirectLineage {
    /// This chunk resulted from the splitting of that other chunk. It must have siblings, somewhere.
    //
    // TODO: document `(parent, siblings)`
    SplitFrom(ChunkId, Vec<ChunkId>),

    /// This chunk resulted from the compaction of these other chunks.
    CompactedFrom(BTreeSet<ChunkId>),

    /// This chunk's data was originally fetched from an RRD manifest.
    ///
    /// Even if it gets garbage collected, it can be re-fetched as needed (as long as the backing
    /// Redap server is still available).
    ReferencedFrom(Arc<RrdManifest>),

    /// This chunk's data was originally logged from volatile memory.
    ///
    /// Once garbage collected, this data will be unrecoverable.
    Volatile,
}

impl std::fmt::Display for ChunkDirectLineage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SplitFrom(chunk_id, sibling_ids) => f.write_fmt(format_args!(
                "split-from:{chunk_id} siblings:[{}]",
                sibling_ids.iter().map(|id| id.to_string()).join(",")
            )),

            Self::CompactedFrom(chunk_ids) => f.write_fmt(format_args!(
                "compacted-from:[{}]",
                chunk_ids.iter().join(", ")
            )),

            // TODO: yeah now i want this memoized even more, or this is a nightmare
            #[expect(clippy::unwrap_used)]
            Self::ReferencedFrom(rrd_manifest) => f.write_fmt(format_args!(
                "origin:{}",
                rrd_manifest.compute_sha256().unwrap() // TODO
            )),

            Self::Volatile => f.write_str("origin:<volatile> (cannot be re-fetched)"),
        }
    }
}

impl From<&ChunkDirectLineageReport> for ChunkDirectLineage {
    fn from(report: &ChunkDirectLineageReport) -> Self {
        fn recurse(report: &ChunkDirectLineageReport) -> ChunkDirectLineage {
            match report {
                ChunkDirectLineageReport::SplitFrom(chunk, siblings) => {
                    ChunkDirectLineage::SplitFrom(
                        chunk.id(),
                        siblings.iter().map(|c| c.id()).collect(),
                    )
                }

                ChunkDirectLineageReport::CompactedFrom(chunks) => {
                    ChunkDirectLineage::CompactedFrom(chunks.keys().copied().collect())
                }

                ChunkDirectLineageReport::ReferencedFrom(rrd_manifest) => {
                    ChunkDirectLineage::ReferencedFrom(rrd_manifest.clone())
                }

                ChunkDirectLineageReport::Volatile => ChunkDirectLineage::Volatile,
            }
        }

        recurse(report)
    }
}

impl ChunkDirectLineage {
    pub fn to_report(&self, store: &ChunkStore) -> Option<ChunkDirectLineageReport> {
        fn recurse(
            store: &ChunkStore,
            lineage: &ChunkDirectLineage,
        ) -> Option<ChunkDirectLineageReport> {
            match lineage {
                ChunkDirectLineage::SplitFrom(chunk_id, sibling_ids) => {
                    let mut siblings = Vec::new();
                    for sibling_id in sibling_ids {
                        siblings.push(store.chunks_per_chunk_id.get(sibling_id).cloned()?);
                    }
                    Some(ChunkDirectLineageReport::SplitFrom(
                        store.chunks_per_chunk_id.get(chunk_id).cloned()?,
                        siblings,
                    ))
                }

                ChunkDirectLineage::CompactedFrom(chunk_ids) => {
                    let mut chunks = BTreeMap::new();
                    for chunk_id in chunk_ids {
                        chunks.insert(*chunk_id, store.chunks_per_chunk_id.get(chunk_id).cloned()?);
                    }
                    Some(ChunkDirectLineageReport::CompactedFrom(chunks))
                }

                ChunkDirectLineage::ReferencedFrom(rrd_manifest) => Some(
                    ChunkDirectLineageReport::ReferencedFrom(rrd_manifest.clone()),
                ),

                ChunkDirectLineage::Volatile => Some(ChunkDirectLineageReport::Volatile),
            }
        }

        recurse(store, self)
    }
}

/// How a chunk relates its direct ancestor(s).
///
/// These ancestors can be other chunk(s) or, at the top of the lineage tree, the origin of where
/// the data came from in the first place (volatile memory vs. an RRD manifest).
///
/// This type always holds strong references towards [`Chunk`]s.
/// This makes it usable in physical contexts where lineage information must ensure that the
/// underlying data remains in local memory, such as when firing store events (so that the data
/// doesn't get garbage collected before every downstream consumer has had a chance to process it).
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkDirectLineageReport {
    /// This chunk resulted from the splitting of that other chunk. It must have siblings, somewhere.
    //
    // TODO: document `(parent, siblings)`
    // TODO: we need to make to make it clear that splitting is a one time op, and will never be
    // deeper than 1
    SplitFrom(Arc<Chunk>, Vec<Arc<Chunk>>),

    /// This chunk resulted from the compaction of these other chunks.
    //
    // TODO: we need to make it clear that compaction is a continuous process and the lineage tree
    // for a compacted chunk is expected to grow deeper as time goes on and new data gets inserted.
    CompactedFrom(BTreeMap<ChunkId, Arc<Chunk>>),

    /// This chunk's data was originally fetched from an RRD manifest.
    ///
    /// Even if it gets garbage collected, it can be re-fetched as needed (as long as the backing
    /// Redap server is still available).
    ReferencedFrom(Arc<RrdManifest>),

    /// This chunk's data was originally logged from volatile memory.
    ///
    /// Once garbage collected, this data will be unrecoverable.
    Volatile,
}

impl ChunkStore {
    /// Formats the complete lineage tree of a chunk in a human readable fashion.
    ///
    /// This is a debugging tool, it makes no effort whatsoever to try and be performant.
    pub fn format_lineage(&self, chunk_id: &ChunkId) -> String {
        fn compute_staticness(store: &ChunkStore, chunk_id: &ChunkId) -> &'static str {
            // If the chunk is physically loaded, then this is trivial to answer.
            if let Some(chunk) = store.chunks_per_chunk_id.get(chunk_id) {
                return if chunk.is_static() { "yes" } else { "no" };
            }

            // OTOH, if it has been offloaded, now we need to track down its roots and determine
            // from an RRD manifest whether it is static or not, if possible.
            for (_, rrd_manifest) in store.find_root_rrd_manifests(chunk_id) {
                for (id, is_static) in itertools::izip!(
                    // flatten the Result<>
                    rrd_manifest.col_chunk_id().into_iter().flatten(),
                    rrd_manifest.col_chunk_is_static().into_iter().flatten(),
                ) {
                    if *chunk_id == id {
                        return if is_static { "yes" } else { "no" };
                    }
                }
            }

            // Otherwise, we simply cannot possibly know anymore.
            "unknown"
        }

        #[expect(clippy::string_add)] // clearer, in this instance
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, depth: usize) -> String {
            let chunk = store.chunks_per_chunk_id.get(chunk_id);

            let lineage = store.chunks_lineage.get(chunk_id);
            let status = if chunk.is_some() {
                "loaded"
            } else {
                "offloaded"
            };
            let is_static = compute_staticness(store, chunk_id);
            let width = (depth + 1) * 4;

            let sibling_ids = match lineage {
                Some(ChunkDirectLineage::SplitFrom(_, sibling_ids)) => sibling_ids.as_slice(),
                _ => &[],
            };

            (if sibling_ids.is_empty() {
                format!("{chunk_id} (status:{status} static:{is_static})\n")
            } else {
                // TODO: probably a bad idea to show all of them (just a count, maybe?)
                let sibling_ids = sibling_ids.iter().map(|id| id.to_string()).join(",");
                format!(
                    "{chunk_id} (status:{status} static:{is_static} siblings: [{sibling_ids}])\n"
                )
            }) + &match lineage {
                Some(ChunkDirectLineage::SplitFrom(id, _sibling_ids)) => {
                    format!(
                        "{:width$}split-from: {}",
                        "",
                        recurse(store, id, depth + 1),
                        width = width
                    )
                }

                Some(ChunkDirectLineage::CompactedFrom(ids)) => ids
                    .iter()
                    .map(|id| {
                        format!(
                            "{:width$}compacted-from: {}",
                            "",
                            recurse(store, id, depth + 1),
                            width = width
                        )
                    })
                    .join("\n"),

                Some(lineage) => format!("{:width$}{lineage}", "", width = width),

                None => format!("{:width$}<invalid>", "", width = width),
            }
        }

        recurse(self, chunk_id, 0)
    }

    /// Returns the roots from which a given chunk was derived from.
    ///
    /// Due to compaction, lineage forms a tree rather than a straight line, and therefore it is
    /// possible (and even common) for a chunk to have more than one root.
    ///
    /// The resulting root chunks might or might not be volatile.
    /// If you only care about chunks that are still available for download, see [`Self::find_root_rrd_manifests`].
    pub fn find_root_chunks(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, roots: &mut Vec<ChunkId>) {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                    recurse(store, chunk_id, roots);
                }

                Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                    for chunk_id in chunk_ids {
                        recurse(store, chunk_id, roots);
                    }
                }

                Some(ChunkDirectLineage::ReferencedFrom(_) | ChunkDirectLineage::Volatile) => {
                    roots.push(*chunk_id);
                }

                _ => {}
            }
        }

        let mut roots = Vec::new();
        recurse(self, chunk_id, &mut roots);

        roots
    }

    /// Returns the top-level non-volatile roots of a given chunk, if any.
    ///
    /// Due to compaction, lineage forms a tree rather than a straight line, and therefore it is
    /// possible (and even common) for a chunk to have more than one root, from possibly more than
    /// one RRD manifest.
    ///
    /// The resulting root chunks are guaranteed to be backed by an RRD manifest (non-volatile).
    /// If you want to find all root chunks regardless of their origin, refer to [`Self::find_root_rrd_manifests`]
    /// instead.
    pub fn find_root_rrd_manifests(&self, chunk_id: &ChunkId) -> Vec<(ChunkId, Arc<RrdManifest>)> {
        fn recurse(
            store: &ChunkStore,
            chunk_id: &ChunkId,
            roots: &mut Vec<(ChunkId, Arc<RrdManifest>)>,
        ) {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                    recurse(store, chunk_id, roots);
                }

                Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                    for chunk_id in chunk_ids {
                        recurse(store, chunk_id, roots);
                    }
                }

                Some(ChunkDirectLineage::ReferencedFrom(rrd_manifest)) => {
                    roots.push((*chunk_id, rrd_manifest.clone()));
                }

                _ => {}
            }
        }

        let mut roots = Vec::new();
        recurse(self, chunk_id, &mut roots);

        roots
    }

    /// Returns true if the specified chunk has at least one ancestor that resulted from a split.
    pub fn has_split_ancestor(&self, chunk_id: &ChunkId) -> bool {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId) -> bool {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(_chunk_id, _sibling_ids)) => true,

                Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                    for chunk_id in chunk_ids {
                        if recurse(store, chunk_id) {
                            return true;
                        }
                    }
                    false
                }

                _ => false,
            }
        }

        recurse(self, chunk_id)
    }

    /// Returns true if the specified chunk has at least one ancestor that resulted from a compaction.
    pub fn has_compacted_ancestor(&self, chunk_id: &ChunkId) -> bool {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId) -> bool {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                    recurse(store, chunk_id)
                }
                Some(ChunkDirectLineage::CompactedFrom(_chunk_ids)) => true,
                _ => false,
            }
        }

        recurse(self, chunk_id)
    }
}

// TODO:
// * test no splitting compacted
// * test no compacting split
// * test that events match expectations and has_xxx_ancestor helpers

#[cfg(test)]
mod tests {
    use re_chunk::{Chunk, EntityPath, RowId, Timeline};
    use re_log_types::StoreId;
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::external::re_tuid::Tuid;

    use crate::ChunkStoreConfig;

    use super::*;

    #[test]
    fn lineage_basics_volatile() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 3,             // !
                chunk_max_rows_if_unsorted: 3, // !
            },
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

        let chunks = [
            build_chunk(1),
            build_chunk(1),
            build_chunk(1),
            build_chunk(1),
            build_chunk(3),
            build_chunk(3),
            build_chunk(6),
        ];

        for chunk in &chunks {
            store.insert_chunk(chunk).unwrap();
        }

        insta::assert_snapshot!("lineage_volatile", generate_redacted_lineage_report(&store));

        for chunk in store.chunks_per_chunk_id.values() {
            assert!(
                store
                    .find_root_chunks(&chunk.id())
                    .into_iter()
                    .all(|root_chunk_id| chunks.iter().any(|c| c.id() == root_chunk_id)),
                "all these chunks' respective roots should come from the starting set"
            );
            assert!(
                store.find_root_rrd_manifests(&chunk.id()).is_empty(),
                "none of these chunks should have a root RRD manifest"
            );
        }
    }

    #[test]
    fn lineage_basics_bootstrapped() {
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

        let chunks = [
            build_chunk(1),
            build_chunk(1),
            build_chunk(1),
            build_chunk(1),
            build_chunk(3),
            build_chunk(3),
            build_chunk(6),
        ];

        let rrd_manifest = {
            let mut store = ChunkStore::new(
                StoreId::recording("app_id", "rec_id"),
                ChunkStoreConfig::ALL_DISABLED,
            );

            for chunk in &chunks {
                store.insert_chunk(chunk).unwrap();
            }

            Arc::new(store_to_rrd_manifest(&store))
        };

        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 3,             // !
                chunk_max_rows_if_unsorted: 3, // !
            },
        );

        // Load it virtually.
        store.insert_rrd_manifest(rrd_manifest.clone()).unwrap();

        // Load it physically.
        for chunk in &chunks {
            store.insert_chunk(chunk).unwrap();
        }

        insta::assert_snapshot!(
            "lineage_bootstrapped",
            generate_redacted_lineage_report(&store)
        );

        for chunk in store.chunks_per_chunk_id.values() {
            assert!(
                store
                    .find_root_chunks(&chunk.id())
                    .into_iter()
                    .all(|root_chunk_id| chunks.iter().any(|c| c.id() == root_chunk_id)),
                "all these chunks' respective roots should come from the starting set"
            );

            for (root_chunk_id, root_manifest) in store.find_root_rrd_manifests(&chunk.id()) {
                assert!(
                    chunks.iter().any(|c| c.id() == root_chunk_id),
                    "all these chunks' respective roots should come from the starting manifest",
                );
                assert_eq!(rrd_manifest, root_manifest);
            }
        }
    }

    // ---

    fn next_chunk_id_generator(prefix: u64) -> impl FnMut() -> re_chunk::ChunkId {
        let mut chunk_id = re_chunk::ChunkId::from_tuid(Tuid::from_nanos_and_inc(prefix, 0));
        move || {
            chunk_id = chunk_id.next();
            chunk_id
        }
    }

    fn generate_redacted_lineage_report(store: &ChunkStore) -> String {
        let mut next_chunk_id = next_chunk_id_generator(1337);
        let redacted_chunk_ids: ahash::HashMap<_, _> = store
            .chunks_lineage
            .keys()
            .map(|chunk_id| (*chunk_id, next_chunk_id()))
            .collect();

        let mut lineage_report = Vec::new();
        for chunk_id in store.chunks_per_chunk_id.keys() {
            lineage_report.push(store.format_lineage(chunk_id));
        }

        let mut lineage_report = lineage_report.join("\n");
        for (chunk_id, redacted_chunk_id) in redacted_chunk_ids {
            lineage_report =
                lineage_report.replace(&chunk_id.to_string(), &redacted_chunk_id.to_string());
        }

        lineage_report
    }

    fn store_to_rrd_manifest(store: &ChunkStore) -> RrdManifest {
        let mut rrd_manifest_builder = re_log_encoding::RrdManifestBuilder::default();

        let mut offset = 0;
        for chunk in store.iter_chunks() {
            let chunk_batch = chunk.to_chunk_batch().unwrap();

            use re_byte_size::SizeBytes as _;
            let byte_size_uncompressed = chunk.heap_size_bytes();

            let uncompressed_byte_span = re_span::Span {
                start: offset,
                len: byte_size_uncompressed,
            };

            offset += byte_size_uncompressed;

            rrd_manifest_builder
                .append(&chunk_batch, uncompressed_byte_span, byte_size_uncompressed)
                .unwrap();
        }

        let rrd_manifest = rrd_manifest_builder.build(store.id()).unwrap();
        rrd_manifest.sanity_check_cheap().unwrap();
        rrd_manifest.sanity_check_heavy().unwrap();

        rrd_manifest
    }
}
