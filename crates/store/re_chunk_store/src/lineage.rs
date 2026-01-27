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
/// Use [`ChunkDirectLineage::to_report`] to generate a [`ChunkDirectLineageReport`] instead.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkDirectLineage {
    /// This chunk resulted from the splitting of that other chunk. It must have siblings, somewhere.
    ///
    /// ### Understanding split semantics
    ///
    /// Splitting is a one-time and one-time only operation that happens during insertion.
    /// A chunk will only be split once, and it will be split in as many parts as needed to comply
    /// with the active thresholds at that time.
    ///
    /// If a chunk is the descendant of either a compacted or split chunk, then it cannot ever be
    /// split further. Therefore, a split chunk's lineage always has a depth of 1.
    /// When looking at a [`crate::ChunkStoreDiff`], that means that a `chunk_before_processing` is
    /// always the unsplit original chunk, while `chunk_after_processing` is always the split
    /// chunk at depth=1.
    ///
    /// Value: `(parent_id, sibling_ids)`.
    SplitFrom(ChunkId, Vec<ChunkId>),

    /// This chunk resulted from the compaction of these other chunks.
    ///
    /// ### Understanding compaction semantics
    ///
    /// Compaction is a continuous process that happens every time a chunk is inserted into the store.
    ///
    /// A compacted chunk can always take part in future compaction events. Therefore, compacted
    /// chunks can have an arbitrary large and deep lineage tree.
    /// Most of that tree will be virtual, since the ancestors of compacted chunks are always
    /// immediately removed from the physical store.
    ///
    /// If a chunk descends from a split, it can never take part in a compaction event again.
    ///
    /// Value: `(parent, siblings)`.
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

impl re_byte_size::SizeBytes for ChunkDirectLineage {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::SplitFrom(chunk_id, chunk_ids) => {
                chunk_id.heap_size_bytes() + chunk_ids.heap_size_bytes()
            }
            Self::CompactedFrom(btree_set) => btree_set.heap_size_bytes(),
            Self::ReferencedFrom(_rrd_manifest) => {
                0 // calculating the size of each RrdManifest over and over again is too slow. It is also amortized, so doesn't matter much.
            }
            Self::Volatile => 0,
        }
    }
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

            Self::ReferencedFrom(rrd_manifest) => {
                // TODO(cmc): recomputing the sha256 is wasteful, but then again this is a display
                // impl for debugging purposes so… eh.
                if let Ok(sha256) = rrd_manifest.compute_sha256() {
                    f.write_fmt(format_args!("origin:{sha256}"))
                } else {
                    f.write_str("origin:<ERROR>")
                }
            }

            Self::Volatile => f.write_str("origin:<volatile> (cannot be re-fetched)"),
        }
    }
}

impl From<ChunkDirectLineageReport> for ChunkDirectLineage {
    fn from(value: ChunkDirectLineageReport) -> Self {
        (&value).into()
    }
}

impl From<&ChunkDirectLineageReport> for ChunkDirectLineage {
    /// This is *not* recursive.
    fn from(report: &ChunkDirectLineageReport) -> Self {
        match report {
            ChunkDirectLineageReport::SplitFrom(chunk, siblings) => {
                Self::SplitFrom(chunk.id(), siblings.iter().map(|c| c.id()).collect())
            }

            ChunkDirectLineageReport::CompactedFrom(chunks) => {
                Self::CompactedFrom(chunks.keys().copied().collect())
            }

            ChunkDirectLineageReport::ReferencedFrom(rrd_manifest) => {
                Self::ReferencedFrom(rrd_manifest.clone())
            }

            ChunkDirectLineageReport::Volatile => Self::Volatile,
        }
    }
}

impl ChunkDirectLineage {
    /// Converts into a [`ChunkDirectLineageReport`]. This it *not* recursive.
    ///
    /// This requires all chunks that appear in the linage tree to be physically loaded in memory.
    /// It will return `None` otherwise.
    pub fn to_report(&self, store: &ChunkStore) -> Option<ChunkDirectLineageReport> {
        match self {
            Self::SplitFrom(chunk_id, sibling_ids) => {
                let mut siblings = Vec::new();
                for sibling_id in sibling_ids {
                    siblings.push(store.chunks_per_chunk_id.get(sibling_id).cloned()?);
                }
                Some(ChunkDirectLineageReport::SplitFrom(
                    store.chunks_per_chunk_id.get(chunk_id).cloned()?,
                    siblings,
                ))
            }

            Self::CompactedFrom(chunk_ids) => {
                let mut chunks = BTreeMap::new();
                for chunk_id in chunk_ids {
                    chunks.insert(*chunk_id, store.chunks_per_chunk_id.get(chunk_id).cloned()?);
                }
                Some(ChunkDirectLineageReport::CompactedFrom(chunks))
            }

            Self::ReferencedFrom(rrd_manifest) => Some(ChunkDirectLineageReport::ReferencedFrom(
                rrd_manifest.clone(),
            )),

            Self::Volatile => Some(ChunkDirectLineageReport::Volatile),
        }
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
/// Use the `Into<ChunkDirectLineage>` to generate a [`ChunkDirectLineage`] instead.
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkDirectLineageReport {
    /// This chunk resulted from the splitting of that other chunk. It must have siblings, somewhere.
    ///
    /// ### Understanding split semantics
    ///
    /// Splitting is a one-time and one-time only operation that happens during insertion.
    /// A chunk will only be split once, and it will be split in as many parts as needed to comply
    /// with the active thresholds at that time.
    ///
    /// If a chunk is the descendant of either a compacted or split chunk, then it cannot ever be
    /// split further. Therefore, a split chunk's lineage always has a depth of 1.
    /// When looking at a [`crate::ChunkStoreDiff`], that means that a `chunk_before_processing` is
    /// always the unsplit original chunk, while `chunk_after_processing` is always the split
    /// chunk at depth=1.
    ///
    /// Value: `(parent, siblings)`.
    SplitFrom(Arc<Chunk>, Vec<Arc<Chunk>>),

    /// This chunk resulted from the compaction of these other chunks.
    ///
    /// ### Understanding compaction semantics
    ///
    /// Compaction is a continuous process that happens every time a chunk is inserted into the store.
    ///
    /// A compacted chunk can always take part in future compaction events. Therefore, compacted
    /// chunks can have an arbitrary large and deep lineage tree.
    /// Most of that tree will be virtual, since the ancestors of compacted chunks are always
    /// immediately removed from the physical store.
    ///
    /// If a chunk descends from a split, it can never take part in a compaction event again.
    ///
    /// Value: `(parent, siblings)`.
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
                let sibling_ids = sibling_ids.iter().map(|id| id.to_string()).join(",");
                format!(
                    "{chunk_id} (status:{status} static:{is_static} siblings:[{sibling_ids}])\n"
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

    /// Returns true if this is a root-level chunk.
    ///
    /// Root-level chunks sit directly at the top of the lineage tree: they cannot be issued from
    /// either a split or a compaction.
    /// I.e. the next layer is necessarily either a reference to volatile memory, or to an RRD
    /// manifest.
    pub fn is_root_chunk(&self, chunk_id: &ChunkId) -> bool {
        let Some(lineage) = self.chunks_lineage.get(chunk_id) else {
            // Only way to fall through here is if the chunk was never inserted to begin with, at
            // which point it must be a root, by definition.
            return true;
        };
        matches!(
            lineage,
            ChunkDirectLineage::ReferencedFrom(_) | ChunkDirectLineage::Volatile
        )
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
        let mut roots = Vec::new();
        self.collect_root_rrd_manifests(chunk_id, &mut roots);

        roots
    }

    /// See [`Self::find_root_rrd_manifests`].
    pub fn collect_root_rrd_manifests(
        &self,
        chunk_id: &ChunkId,
        roots: &mut Vec<(ChunkId, Arc<RrdManifest>)>,
    ) {
        let lineage = self.chunks_lineage.get(chunk_id);
        match lineage {
            Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                self.collect_root_rrd_manifests(chunk_id, roots);
            }

            Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                for chunk_id in chunk_ids {
                    self.collect_root_rrd_manifests(chunk_id, roots);
                }
            }

            Some(ChunkDirectLineage::ReferencedFrom(rrd_manifest)) => {
                roots.push((*chunk_id, rrd_manifest.clone()));
            }

            _ => {}
        }
    }

    /// Iterates over all physical chunks that have this chunk
    /// as an ancestor.
    pub fn collect_physical_descendents_of(
        &self,
        chunk_id: &ChunkId,
        descendents: &mut Vec<ChunkId>,
    ) {
        let is_physical = |c: &&ChunkId| self.chunks_per_chunk_id.contains_key(c);

        if is_physical(&chunk_id) {
            descendents.push(*chunk_id);
        } else if let Some(split_chunks) = self.dangling_splits.get(chunk_id) {
            descendents.extend(split_chunks.iter().filter(is_physical).copied());
        } else {
            let mut source_id = *chunk_id;

            let compacted = loop {
                let Some(chunk_id) = self.leaky_compactions.get(&source_id) else {
                    break None;
                };

                if is_physical(&chunk_id) {
                    break Some(*chunk_id);
                }

                source_id = *chunk_id;
            };

            if let Some(chunk_id) = compacted {
                descendents.push(chunk_id);
            }
        }
    }

    /// Returns true if either the specified chunk or one of its ancestors resulted from a split.
    pub fn descends_from_a_split(&self, chunk_id: &ChunkId) -> bool {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, compaction_found: bool) -> bool {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(_chunk_id, _sibling_ids)) => {
                    #[expect(clippy::manual_assert)]
                    if cfg!(debug_assertions) && compaction_found {
                        panic!(
                            "Chunk {chunk_id} mixes compaction and splitting in its lineage tree"
                        )
                    }
                    true
                }

                Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                    for chunk_id in chunk_ids {
                        if recurse(store, chunk_id, true) {
                            return true;
                        }
                    }
                    false
                }

                _ => false,
            }
        }

        let compaction_found = false;
        recurse(self, chunk_id, compaction_found)
    }

    /// Returns true if either the specified chunk or one of its ancestors resulted from a compaction.
    pub fn descends_from_a_compaction(&self, chunk_id: &ChunkId) -> bool {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, split_found: bool) -> bool {
            let lineage = store.chunks_lineage.get(chunk_id);
            match lineage {
                Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                    recurse(store, chunk_id, true)
                }

                Some(ChunkDirectLineage::CompactedFrom(_chunk_ids)) => {
                    #[expect(clippy::manual_assert)]
                    if cfg!(debug_assertions) && split_found {
                        panic!(
                            "Chunk {chunk_id} mixes compaction and splitting in its lineage tree"
                        )
                    }
                    true
                }

                _ => false,
            }
        }

        let split_found = false;
        recurse(self, chunk_id, split_found)
    }

    /// Returns the direct lineage of a chunk.
    pub fn direct_lineage(&self, chunk_id: &ChunkId) -> Option<&ChunkDirectLineage> {
        self.chunks_lineage.get(chunk_id)
    }
}

#[cfg(test)]
#[expect(clippy::bool_assert_comparison)] // I like it that way, sue me
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
            let events = store.insert_chunk(chunk).unwrap();
            for event in events {
                let diff = event.to_addition().unwrap();
                if let ChunkDirectLineageReport::SplitFrom(src, _siblings) = &diff.direct_lineage {
                    assert_eq!(
                        diff.chunk_before_processing.id(),
                        src.id(),
                        "splits are guaranteed flat, and therefore the origin of a split should always match the unprocessed chunk",
                    );
                }
            }
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

        let store_id = StoreId::recording("app_id", "rec_id");
        let rrd_manifest =
            RrdManifest::build_in_memory_from_chunks(store_id.clone(), chunks.iter().map(|c| &**c))
                .unwrap();
        let mut store = ChunkStore::new(
            store_id,
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
            let events = store.insert_chunk(chunk).unwrap();
            for event in events {
                let diff = event.to_addition().unwrap();
                if let ChunkDirectLineageReport::SplitFrom(src, _siblings) = &diff.direct_lineage {
                    assert_eq!(
                        diff.chunk_before_processing.id(),
                        src.id(),
                        "splits are guaranteed flat, and therefore the origin of a split should always match the unprocessed chunk",
                    );
                }
            }
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

    #[test]
    fn lineage_dangling_splits() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 1,             // !
                chunk_max_rows_if_unsorted: 1, // !
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

        let chunk = build_chunk(4);

        // We will end up with 4 split chunks.
        let events = store.insert_chunk(&chunk).unwrap();
        assert_eq!(4, events.len());
        for event in &events {
            assert_eq!(true, event.is_addition());

            // Check that splits are always flattened, very important!
            let siblings = events
                .iter()
                .filter(|e| e.delta_chunk().unwrap().id() != event.delta_chunk().unwrap().id())
                .map(|e| e.delta_chunk().unwrap().clone())
                .collect_vec();
            assert_eq!(
                ChunkDirectLineageReport::SplitFrom(chunk.clone(), siblings),
                event.to_addition().unwrap().direct_lineage,
            );
        }

        assert_eq!(4, store.num_physical_chunks());
        for chunk in store.iter_physical_chunks() {
            assert_eq!(true, store.descends_from_a_split(&chunk.id()));
            assert_eq!(false, store.descends_from_a_compaction(&chunk.id()));
        }

        // GC 50% of the store. We're left with 2 split chunks.
        let (events, _) = store.gc(&crate::GarbageCollectionOptions {
            target: crate::GarbageCollectionTarget::DropAtLeastFraction(0.5),
            time_budget: std::time::Duration::MAX,
            protect_latest: 0,
            protected_time_ranges: Default::default(),
            protected_chunks: Default::default(),
            furthest_from: None,
            perform_deep_deletions: false,
        });
        assert_eq!(2, events.len());
        for event in events {
            assert_eq!(true, event.is_deletion());
        }

        assert_eq!(2, store.num_physical_chunks());
        for chunk in store.iter_physical_chunks() {
            assert_eq!(true, store.descends_from_a_split(&chunk.id()));
            assert_eq!(false, store.descends_from_a_compaction(&chunk.id()));
        }

        // Now re-insert the original chunk. The store should detect this, and clear all the
        // dangling splits accordingly.
        // Therefore we end up with 4 chunks in the store once again, not 6.
        let events = store.insert_chunk(&chunk).unwrap();
        assert_eq!(6, events.len());
        for event in &events[..2] {
            assert_eq!(true, event.is_deletion()); // dangling splits
        }
        for event in &events[2..] {
            assert_eq!(true, event.is_addition()); // new splits
        }

        assert_eq!(4, store.num_physical_chunks());
        for chunk in store.iter_physical_chunks() {
            assert_eq!(true, store.descends_from_a_split(&chunk.id()));
            assert_eq!(false, store.descends_from_a_compaction(&chunk.id()));
        }
    }

    #[test]
    fn splits_cannot_compact() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 10,             // !
                chunk_max_rows_if_unsorted: 10, // !
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

        let chunk1 = build_chunk(12);
        let chunk2 = build_chunk(1);
        let chunk3 = build_chunk(2);

        // We will end up with 2 split chunks, both below the num_rows threshold.
        let events = store.insert_chunk(&chunk1).unwrap();
        assert_eq!(2, events.len());
        for event in events {
            assert_eq!(true, event.is_addition());
        }

        assert_eq!(2, store.num_physical_chunks());
        for chunk in store.iter_physical_chunks() {
            assert_eq!(true, store.descends_from_a_split(&chunk.id()));
            assert_eq!(false, store.descends_from_a_compaction(&chunk.id()));
        }

        // This should not get compacted with anything, since splits cannot compact.
        let events = store.insert_chunk(&chunk2).unwrap();
        assert_eq!(1, events.len());
        for event in events {
            assert_eq!(true, event.is_addition());
        }

        assert_eq!(3, store.num_physical_chunks());
        {
            let chunk_ids = store.chunk_ids_per_min_row_id.values().collect_vec();

            assert_eq!(true, store.descends_from_a_split(chunk_ids[0]));
            assert_eq!(false, store.descends_from_a_compaction(chunk_ids[0]));

            assert_eq!(true, store.descends_from_a_split(chunk_ids[1]));
            assert_eq!(false, store.descends_from_a_compaction(chunk_ids[1]));

            assert_eq!(false, store.descends_from_a_split(chunk_ids[2]));
            assert_eq!(false, store.descends_from_a_compaction(chunk_ids[2]));
        }

        // This should get compacted with chunk2, OTOH.
        let events = store.insert_chunk(&chunk3).unwrap();
        assert_eq!(1, events.len());
        assert_eq!(true, events[0].is_addition());
        assert_eq!(&chunk3, events[0].delta_chunk().unwrap());
        assert_eq!(
            ChunkDirectLineageReport::CompactedFrom(
                [(chunk2.id(), chunk2.clone()), (chunk3.id(), chunk3.clone())]
                    .into_iter()
                    .collect()
            ),
            events[0].to_addition().unwrap().direct_lineage
        );

        assert_eq!(3, store.num_physical_chunks());
        {
            let chunk_ids = store.chunk_ids_per_min_row_id.values().collect_vec();

            assert_eq!(true, store.descends_from_a_split(chunk_ids[0]));
            assert_eq!(false, store.descends_from_a_compaction(chunk_ids[0]));

            assert_eq!(true, store.descends_from_a_split(chunk_ids[1]));
            assert_eq!(false, store.descends_from_a_compaction(chunk_ids[1]));

            assert_eq!(false, store.descends_from_a_split(chunk_ids[2]));
            assert_eq!(true, store.descends_from_a_compaction(chunk_ids[2]));
        }
    }

    #[test]
    fn compacted_cannot_split() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 10,             // !
                chunk_max_rows_if_unsorted: 10, // !
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

        let chunk1 = build_chunk(9);
        let chunk2 = build_chunk(9);

        let events = store.insert_chunk(&chunk1).unwrap();
        assert_eq!(1, events.len());
        for event in events {
            assert_eq!(true, event.is_addition());
        }
        let events = store.insert_chunk(&chunk2).unwrap();
        assert_eq!(1, events.len());
        for event in events {
            assert_eq!(true, event.is_addition());
        }

        // The chunks should just not get compacted since the result would be beyond the num_rows
        // threshold, and therefore will never be split either since there will never be a chunk
        // larger than the threshold in the first place.
        assert_eq!(2, store.num_physical_chunks());
        assert_eq!(
            vec![chunk1.id(), chunk2.id()],
            store.chunks_per_chunk_id.keys().copied().collect_vec()
        );
        for chunk in store.iter_physical_chunks() {
            assert_eq!(false, store.descends_from_a_split(&chunk.id()));
            assert_eq!(false, store.descends_from_a_compaction(&chunk.id()));
        }
    }

    #[test]
    fn linear_recursive_compaction() {
        let mut store = ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 10,             // !
                chunk_max_rows_if_unsorted: 10, // !
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

        let chunks = (0..10).map(|_| build_chunk(1)).collect_vec();

        let mut prev_chunk: Option<Arc<Chunk>> = None;
        for chunk in chunks {
            let mut events = store.insert_chunk(&chunk).unwrap();
            assert_eq!(1, events.len());

            let event = events.pop().unwrap();
            let event = event.to_addition().unwrap();
            assert_eq!(chunk.id(), event.chunk_before_processing.id());

            assert_eq!(
                false,
                store.descends_from_a_split(&event.chunk_before_processing.id())
            );
            assert_eq!(
                false,
                store.descends_from_a_compaction(&event.chunk_before_processing.id())
            );
            assert_eq!(
                false,
                store.descends_from_a_split(&event.chunk_after_processing.id())
            );

            if let Some(prev_chunk) = prev_chunk.take() {
                let lineage: ChunkDirectLineage = event.direct_lineage.clone().into();
                let expected = ChunkDirectLineage::CompactedFrom(
                    [chunk.id(), prev_chunk.id()].into_iter().collect(),
                );
                assert_eq!(expected, lineage);
                assert_eq!(
                    true,
                    store.descends_from_a_compaction(&event.chunk_after_processing.id())
                );
            } else {
                let lineage: ChunkDirectLineage = event.direct_lineage.clone().into();
                let expected = ChunkDirectLineage::Volatile;
                assert_eq!(expected, lineage);
                assert_eq!(
                    false,
                    store.descends_from_a_compaction(&event.chunk_after_processing.id())
                );
            }

            prev_chunk = Some(event.chunk_after_processing.clone());
        }

        assert_eq!(1, store.num_physical_chunks());
    }

    #[test]
    fn lineage_leaky_compactions() {
        let store_id = StoreId::recording("app_id", "rec_id");
        let mut store = ChunkStore::new(
            store_id.clone(),
            ChunkStoreConfig {
                enable_changelog: false, // irrelevant
                chunk_max_bytes: u64::MAX,
                chunk_max_rows: 10,             // !
                chunk_max_rows_if_unsorted: 10, // !
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

        let chunk1 = build_chunk(1);
        let chunk2 = build_chunk(1);
        let chunk3 = build_chunk(1);

        // The store should realize that these recurring compactions all refer to the same data,
        // and therefore not actually do anything past the first iteration.
        for _ in 0..3 {
            store.insert_chunk(&chunk1).unwrap();
            store.insert_chunk(&chunk2).unwrap();
            store.insert_chunk(&chunk3).unwrap();
        }

        assert_eq!(1, store.num_physical_chunks());
        insta::assert_snapshot!(
            "lineage_leaky_compactions",
            generate_redacted_lineage_report(&store)
        );
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
}
