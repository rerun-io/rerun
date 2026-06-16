use std::collections::BTreeMap;
use std::sync::Arc;

use itertools::{Either, Itertools as _};

use re_chunk::{Chunk, ChunkId};

use crate::ChunkStore;

// ---

#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub(crate) struct TrackedDirectChunkLineage {
    pub(crate) lineage: ChunkDirectLineage,

    /// How many other [`TrackedDirectChunkLineage`] or physical chunks that
    /// reference this lineage.
    pub(crate) ref_count: u32,
    pub(crate) descends_from_manifest: bool,
}

impl std::ops::Deref for TrackedDirectChunkLineage {
    type Target = ChunkDirectLineage;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.lineage
    }
}

impl std::ops::DerefMut for TrackedDirectChunkLineage {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lineage
    }
}

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
#[derive(Clone, PartialEq, Eq, re_byte_size::SizeBytes)]
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
    SplitFrom(ChunkId, Box<[ChunkId]>),

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
    /// Value: `parents`.
    CompactedFrom(Box<[ChunkId]>),

    /// This chunk's data was originally fetched from an RRD manifest.
    ///
    /// Even if it gets garbage collected, it can be re-fetched as needed (as long as the backing
    /// Redap server is still available).
    RootFromManifest { is_static: bool },

    /// This chunk's data was originally logged from volatile memory.
    ///
    /// Once garbage collected, this data will be unrecoverable.
    Volatile,
}

impl std::fmt::Debug for ChunkDirectLineage {
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

            Self::RootFromManifest { is_static } => {
                write!(f, "origin:(static: {is_static})")
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

            ChunkDirectLineageReport::RootFromManifest { is_static } => Self::RootFromManifest {
                is_static: *is_static,
            },

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
                    siblings.push(
                        store
                            .physical_chunks_per_chunk_id
                            .get(sibling_id)
                            .cloned()?,
                    );
                }
                Some(ChunkDirectLineageReport::SplitFrom(
                    store.physical_chunks_per_chunk_id.get(chunk_id).cloned()?,
                    siblings,
                ))
            }

            Self::CompactedFrom(chunk_ids) => {
                let mut chunks = BTreeMap::new();
                for chunk_id in chunk_ids {
                    chunks.insert(
                        *chunk_id,
                        store.physical_chunks_per_chunk_id.get(chunk_id).cloned()?,
                    );
                }
                Some(ChunkDirectLineageReport::CompactedFrom(chunks))
            }

            Self::RootFromManifest { is_static } => {
                Some(ChunkDirectLineageReport::RootFromManifest {
                    is_static: *is_static,
                })
            }

            Self::Volatile => Some(ChunkDirectLineageReport::Volatile),
        }
    }

    pub fn iter_referenced_chunks(&self) -> impl Iterator<Item = &ChunkId> {
        match self {
            Self::SplitFrom(chunk_id, _) => Some(Either::Left(std::iter::once(chunk_id))),
            Self::CompactedFrom(chunks) => Some(Either::Right(chunks.iter())),
            Self::RootFromManifest { .. } | Self::Volatile => None,
        }
        .into_iter()
        .flatten()
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
#[derive(Clone, PartialEq)]
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
    /// Value: parents
    CompactedFrom(BTreeMap<ChunkId, Arc<Chunk>>),

    /// This chunk's data was originally fetched from an RRD manifest.
    ///
    /// Even if it gets garbage collected, it can be re-fetched as needed (as long as the backing
    /// Redap server is still available).
    RootFromManifest { is_static: bool },

    /// This chunk's data was originally logged from volatile memory.
    ///
    /// Once garbage collected, this data will be unrecoverable.
    Volatile,
}

impl std::fmt::Debug for ChunkDirectLineageReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SplitFrom(parent, siblings) => f
                .debug_struct("SplitFrom")
                .field("parent", &parent.id())
                .field("siblings", &siblings.iter().map(|c| c.id()).format(", "))
                .finish(),
            Self::CompactedFrom(map) => f
                .debug_map()
                .entries(map.iter().map(|(k, v)| (k, v.id())))
                .finish(),
            Self::RootFromManifest { is_static } => {
                write!(f, "RootFromManifest(static: {is_static})")
            }
            Self::Volatile => write!(f, "Volatile"),
        }
    }
}

impl ChunkStore {
    /// Formats the complete lineage tree of a chunk in a human readable fashion.
    ///
    /// This is a debugging tool, it makes no effort whatsoever to try and be performant.
    pub fn format_lineage(&self, chunk_id: &ChunkId) -> String {
        fn compute_staticness(store: &ChunkStore, chunk_id: &ChunkId) -> &'static str {
            // If the chunk is physically loaded, then this is trivial to answer.
            if let Some(chunk) = store.physical_chunks_per_chunk_id.get(chunk_id) {
                return if chunk.is_static() { "yes" } else { "no" };
            }

            // OTOH, if it has been offloaded, now we need to track down its roots and determine
            // whether it is static or not from the lineage info.
            for root_id in store.find_root_manifest_chunks(chunk_id) {
                if let Some(ChunkDirectLineage::RootFromManifest { is_static }) =
                    store.chunks_lineage.get(&root_id).map(|l| &l.lineage)
                {
                    return if *is_static { "yes" } else { "no" };
                }
            }

            // Otherwise, we simply cannot possibly know anymore.
            "unknown"
        }

        #[expect(clippy::string_add)] // clearer, in this instance
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, depth: usize) -> String {
            let chunk = store.physical_chunks_per_chunk_id.get(chunk_id);

            let lineage = store.chunks_lineage.get(chunk_id).map(|l| &l.lineage);
            let status = if chunk.is_some() {
                "loaded"
            } else {
                "offloaded"
            };
            let is_static = compute_staticness(store, chunk_id);
            let width = (depth + 1) * 4;

            let sibling_ids = match lineage {
                Some(ChunkDirectLineage::SplitFrom(_, sibling_ids)) => &**sibling_ids,
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

                Some(lineage) => format!("{:width$}{lineage:?}", "", width = width),

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
            lineage.lineage,
            ChunkDirectLineage::RootFromManifest { .. } | ChunkDirectLineage::Volatile
        )
    }

    /// Returns the roots from which a given chunk was derived from.
    ///
    /// Due to compaction, lineage forms a tree rather than a straight line, and therefore it is
    /// possible (and even common) for a chunk to have more than one root.
    ///
    /// The resulting root chunks might or might not be volatile.
    /// If you only care about chunks that are still available for download, see [`Self::find_root_manifest_chunks`].
    pub fn find_root_chunks(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let mut roots = Vec::new();
        self.collect_root_ids(chunk_id, &mut roots);
        roots
    }

    /// See [`Self::find_root_chunks`].
    pub fn collect_root_ids(&self, chunk_id: &ChunkId, roots: &mut Vec<ChunkId>) {
        let lineage = self.chunks_lineage.get(chunk_id).map(|l| &l.lineage);
        match lineage {
            Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                self.collect_root_ids(chunk_id, roots);
            }

            Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                for chunk_id in chunk_ids {
                    self.collect_root_ids(chunk_id, roots);
                }
            }

            Some(ChunkDirectLineage::RootFromManifest { .. } | ChunkDirectLineage::Volatile) => {
                roots.push(*chunk_id);
            }

            None => {}
        }
    }

    /// Returns the top-level non-volatile roots of a given chunk, if any.
    ///
    /// Due to compaction, lineage forms a tree rather than a straight line, and therefore it is
    /// possible (and even common) for a chunk to have more than one root, from possibly more than
    /// one RRD manifest.
    ///
    /// The resulting root chunks are guaranteed to be backed by an RRD manifest (non-volatile).
    /// If you want to find all root chunks regardless of their origin, refer to [`Self::find_root_chunks`]
    /// instead.
    pub fn find_root_manifest_chunks(&self, chunk_id: &ChunkId) -> Vec<ChunkId> {
        let mut roots = Vec::new();
        self.collect_root_manifest_chunks(chunk_id, &mut roots);
        roots
    }

    /// See [`Self::find_root_manifest_chunks`].
    fn collect_root_manifest_chunks(&self, chunk_id: &ChunkId, roots: &mut Vec<ChunkId>) {
        let lineage = self.chunks_lineage.get(chunk_id).map(|l| &l.lineage);
        match lineage {
            Some(ChunkDirectLineage::SplitFrom(chunk_id, _sibling_ids)) => {
                self.collect_root_manifest_chunks(chunk_id, roots);
            }

            Some(ChunkDirectLineage::CompactedFrom(chunk_ids)) => {
                for chunk_id in chunk_ids {
                    self.collect_root_manifest_chunks(chunk_id, roots);
                }
            }

            Some(ChunkDirectLineage::RootFromManifest { .. }) => {
                roots.push(*chunk_id);
            }

            _ => {}
        }
    }

    /// Collects all physical chunks that descend from the given chunk in some way.
    pub fn collect_physical_descendents_of(
        &self,
        chunk_id: &ChunkId,
        descendents: &mut Vec<ChunkId>,
    ) {
        let is_physical = |c: &&ChunkId| self.physical_chunks_per_chunk_id.contains_key(c);

        if is_physical(&chunk_id) {
            // A physical chunk cannot have descendents. If it did, it would have
            // been offloaded already.
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

    /// Returns true if either the specified chunk or one of its ancestors is from a manifest.
    pub fn descends_from_manifest(&self, chunk: &ChunkId) -> bool {
        self.chunks_lineage
            .get(chunk)
            .is_some_and(|l| l.descends_from_manifest)
    }

    /// Returns true if either the specified chunk or one of its ancestors resulted from a split.
    pub fn descends_from_a_split(&self, chunk_id: &ChunkId) -> bool {
        if cfg!(debug_assertions) {
            // Do a bit more expensive recursion as a form of sanity checking:
            fn recurse(store: &ChunkStore, chunk_id: &ChunkId, compaction_found: bool) -> bool {
                let lineage = store.chunks_lineage.get(chunk_id).map(|l| &l.lineage);
                match lineage {
                    Some(ChunkDirectLineage::SplitFrom(_chunk_id, _sibling_ids)) => {
                        re_log::debug_assert!(
                            !compaction_found,
                            "Chunk {chunk_id} mixes compaction and splitting in its lineage tree"
                        );
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
        } else {
            // We never mix splits and compactions in the same lineage tree,
            // so no need to recurse:
            matches!(
                self.chunks_lineage.get(chunk_id).map(|l| &l.lineage),
                Some(ChunkDirectLineage::SplitFrom { .. })
            )
        }
    }

    /// Returns true if either the specified chunk or one of its ancestors resulted from a compaction.
    pub fn descends_from_a_compaction(&self, chunk_id: &ChunkId) -> bool {
        fn recurse(store: &ChunkStore, chunk_id: &ChunkId, split_found: bool) -> bool {
            let lineage = store.chunks_lineage.get(chunk_id).map(|l| &l.lineage);
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
        self.chunks_lineage.get(chunk_id).map(|l| &l.lineage)
    }
}

#[cfg(test)]
#[expect(clippy::bool_assert_comparison)] // I like it that way, sue me
mod tests {
    use re_chunk::{Chunk, EntityPath, RowId, Timeline};
    use re_log_encoding::RrdManifest;
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
            for diff in events.iter().filter_map(|event| event.to_addition()) {
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

        for chunk in store.physical_chunks_per_chunk_id.values() {
            assert!(
                store
                    .find_root_chunks(&chunk.id())
                    .into_iter()
                    .all(|root_chunk_id| chunks.iter().any(|c| c.id() == root_chunk_id)),
                "all these chunks' respective roots should come from the starting set"
            );
            assert!(
                store.find_root_manifest_chunks(&chunk.id()).is_empty(),
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
        let _ignored_events = store.insert_rrd_manifest(rrd_manifest.clone());

        // Load it physically.
        for chunk in &chunks {
            let events = store.insert_chunk(chunk).unwrap();
            assert!(
                events.len() > 1,
                "removal of the ghost index + insertion(s) for the physical chunk, got {events:#?}"
            );

            {
                let diff = events[0].to_deletion().unwrap();
                assert_eq!(chunk.id(), diff.chunk.id(), "ghost index");
            }

            for diff in events
                .iter()
                .filter_map(|event| event.to_addition())
                .skip(1)
            {
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

        for chunk in store.physical_chunks_per_chunk_id.values() {
            assert!(
                store
                    .find_root_chunks(&chunk.id())
                    .into_iter()
                    .all(|root_chunk_id| chunks.iter().any(|c| c.id() == root_chunk_id)),
                "all these chunks' respective roots should come from the starting set"
            );

            for root_chunk_id in store.find_root_manifest_chunks(&chunk.id()) {
                assert!(
                    chunks.iter().any(|c| c.id() == root_chunk_id),
                    "all these chunks' respective roots should come from the starting manifest",
                );
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
        assert_eq!(5, events.len());
        assert!(
            events[4].is_schema_addition(),
            "the first write should emit a schema addition for newly seen columns"
        );
        for event in &events[..4] {
            assert_eq!(true, event.is_addition());

            // Check that splits are always flattened, very important!
            let siblings = events[..4]
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
        assert_eq!(3, events.len());
        for event in &events[..2] {
            assert_eq!(true, event.is_addition());
        }
        assert!(
            events[2].is_schema_addition(),
            "the first write should emit a schema addition for newly seen columns"
        );

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
            let chunk_ids = store
                .physical_chunk_ids_per_min_row_id
                .iter()
                .map(|(_, id)| id)
                .collect_vec();

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
            let chunk_ids = store
                .physical_chunk_ids_per_min_row_id
                .iter()
                .map(|(_, id)| id)
                .collect_vec();

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
        assert_eq!(2, events.len());
        assert_eq!(true, events[0].is_addition());
        assert!(
            events[1].is_schema_addition(),
            "the first write should emit a schema addition for newly seen columns"
        );

        let events = store.insert_chunk(&chunk2).unwrap();
        assert_eq!(1, events.len());
        assert_eq!(true, events[0].is_addition());

        // The chunks should just not get compacted since the result would be beyond the num_rows
        // threshold, and therefore will never be split either since there will never be a chunk
        // larger than the threshold in the first place.
        assert_eq!(2, store.num_physical_chunks());
        assert_eq!(
            vec![chunk1.id(), chunk2.id()],
            store
                .physical_chunks_per_chunk_id
                .keys()
                .copied()
                .collect_vec()
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
        let mut is_first_insert = true;
        for chunk in chunks {
            let mut events = store.insert_chunk(&chunk).unwrap();
            if is_first_insert {
                assert_eq!(2, events.len());
                assert!(
                    events[1].is_schema_addition(),
                    "the first write should emit a schema addition for newly seen columns"
                );
                is_first_insert = false;
            } else {
                assert_eq!(1, events.len());
            }

            let event = events.remove(0);
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

            let lineage: ChunkDirectLineage = event.direct_lineage.clone().into();
            if let Some(prev_chunk) = prev_chunk.take() {
                // `CompactedFrom` keeps its parents sorted by `ChunkId` (it is built from the
                // report's `BTreeMap`), so canonicalize the expected ids the same way.
                let expected = ChunkDirectLineage::CompactedFrom(
                    [chunk.id(), prev_chunk.id()]
                        .into_iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                );
                assert_eq!(expected, lineage);
                assert_eq!(
                    true,
                    store.descends_from_a_compaction(&event.chunk_after_processing.id())
                );
            } else {
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

    // --- ref-counting ---

    #[test]
    fn ref_count_single_volatile_chunk() {
        let mut store = temporal_store(10);
        let mut make_chunk = chunk_factory();

        let chunk = make_chunk(1);
        store.insert_chunk(&chunk).unwrap();

        {
            let lineage = store
                .chunks_lineage
                .get(&chunk.id())
                .expect("a physical chunk must always be tracked");
            assert_eq!(lineage.ref_count, 1, "a single physical reference");
            assert_eq!(lineage.descends_from_manifest, false);
            assert!(matches!(lineage.lineage, ChunkDirectLineage::Volatile));
        }
        assert_eq!(true, store.is_root_chunk(&chunk.id()));
        assert_store_invariants(&store);

        // A volatile root has no manifest to fall back on, so once nothing references it anymore
        // its lineage is dropped rather than kept around forever.
        store.gc(&crate::GarbageCollectionOptions::gc_everything());
        assert_eq!(0, store.num_physical_chunks());
        assert!(
            !store.chunks_lineage.contains_key(&chunk.id()),
            "an unreferenced volatile lineage must not linger after GC",
        );
    }

    #[test]
    fn ref_count_compaction_protects_sources() {
        let mut store = temporal_store(10);
        let mut make_chunk = chunk_factory();

        let chunk_a = make_chunk(1);
        let chunk_b = make_chunk(1);

        store.insert_chunk(&chunk_a).unwrap();
        // The two tiny chunks fold into a single compacted chunk.
        let events = store.insert_chunk(&chunk_b).unwrap();

        let compacted = events
            .iter()
            .find_map(|event| event.to_addition())
            .expect("the compaction must emit an addition")
            .chunk_after_processing
            .clone();
        let compacted_id = compacted.id();

        assert_eq!(1, store.num_physical_chunks());
        assert_eq!(
            vec![compacted_id],
            store
                .physical_chunks_per_chunk_id
                .keys()
                .copied()
                .collect_vec(),
        );

        // The compacted chunk holds the only physical reference and points at both sources.
        {
            let lineage = &store.chunks_lineage[&compacted_id];
            assert_eq!(lineage.ref_count, 1, "a single physical reference");
            let referenced: ahash::HashSet<ChunkId> =
                lineage.iter_referenced_chunks().copied().collect();
            assert_eq!(
                referenced,
                [chunk_a.id(), chunk_b.id()].into_iter().collect(),
            );
            assert_eq!(true, store.descends_from_a_compaction(&compacted_id));
            assert_eq!(false, store.is_root_chunk(&compacted_id));
        }

        // Both sources are physically gone, but their lineage is kept alive by the compacted chunk
        // that now carries their data.
        for src in [&chunk_a, &chunk_b] {
            assert_eq!(
                false,
                store.physical_chunks_per_chunk_id.contains_key(&src.id()),
            );
            let lineage = &store.chunks_lineage[&src.id()];
            assert_eq!(lineage.ref_count, 1, "kept alive by the compacted chunk");
            assert_eq!(true, store.is_root_chunk(&src.id()));
        }

        // Both sources resolve to the compacted result, so re-inserting the same data is a no-op.
        assert_eq!(
            Some(&compacted_id),
            store.leaky_compactions.get(&chunk_a.id()),
        );
        assert_eq!(
            Some(&compacted_id),
            store.leaky_compactions.get(&chunk_b.id()),
        );

        assert_store_invariants(&store);
    }

    #[test]
    fn compacted_lineage_fully_reclaimed_on_gc() {
        let mut store = temporal_store(10);
        let mut make_chunk = chunk_factory();

        // A handful of tiny chunks fold into one compacted chunk, leaving a whole lineage tree
        // behind it.
        for _ in 0..4 {
            store.insert_chunk(&make_chunk(1)).unwrap();
        }
        assert_eq!(1, store.num_physical_chunks());
        assert!(
            store.chunks_lineage.len() > 1,
            "the compacted sources should still be tracked",
        );
        assert!(!store.leaky_compactions.is_empty());
        assert_store_invariants(&store);

        // Dropping the one physical chunk must cascade through the whole tree and reclaim it all,
        // otherwise the bookkeeping maps grow without bound over a long session.
        store.gc(&crate::GarbageCollectionOptions::gc_everything());
        assert_eq!(0, store.num_physical_chunks());
        assert!(
            store.chunks_lineage.is_empty(),
            "compacted lineage tree leaked: {:?}",
            store.chunks_lineage,
        );
        assert!(
            store.leaky_compactions.is_empty(),
            "leaky-compaction tracker leaked: {:?}",
            store.leaky_compactions,
        );
        assert_store_invariants(&store);
    }

    #[test]
    fn split_parent_ref_count_and_reclamation() {
        let mut store = temporal_store(1); // force every row into its own chunk
        let mut make_chunk = chunk_factory();

        let parent = make_chunk(4);
        store.insert_chunk(&parent).unwrap();

        // We end up with four split children, and the parent itself is never physically stored.
        assert_eq!(4, store.num_physical_chunks());
        assert_eq!(
            false,
            store
                .physical_chunks_per_chunk_id
                .contains_key(&parent.id()),
        );

        // The parent's lineage is kept alive by one reference per child.
        {
            let lineage = store
                .chunks_lineage
                .get(&parent.id())
                .expect("the split parent must stay tracked while its children live");
            assert_eq!(lineage.ref_count, 4);
        }
        assert_eq!(true, store.split_on_ingest.contains(&parent.id()));
        assert_eq!(
            Some(4),
            store.dangling_splits.get(&parent.id()).map(|s| s.len()),
        );

        for child in store.iter_physical_chunks() {
            let lineage = &store.chunks_lineage[&child.id()];
            assert_eq!(
                lineage.ref_count, 1,
                "each child holds one physical reference"
            );
            assert_eq!(true, store.descends_from_a_split(&child.id()));
        }
        assert_store_invariants(&store);

        // Reclaiming the children must take the parent's bookkeeping down with them.
        store.gc(&crate::GarbageCollectionOptions::gc_everything());
        assert_eq!(0, store.num_physical_chunks());
        assert!(
            !store.chunks_lineage.contains_key(&parent.id()),
            "split parent lineage leaked",
        );
        assert_eq!(false, store.split_on_ingest.contains(&parent.id()));
        assert_eq!(false, store.dangling_splits.contains_key(&parent.id()));
        assert_store_invariants(&store);
    }

    #[test]
    fn manifest_lineage_survives_gc() {
        let store_id = StoreId::recording("app_id", "rec_id");
        let mut make_chunk = chunk_factory();
        let chunk = make_chunk(1);

        let rrd_manifest =
            RrdManifest::build_in_memory_from_chunks(store_id.clone(), std::iter::once(&*chunk))
                .unwrap();
        let mut store = ChunkStore::new(store_id, temporal_config(10));

        // Load it virtually: the chunk is tracked and flagged as descending from a manifest, but
        // it isn't physically present yet.
        let _ignored_events = store.insert_rrd_manifest(rrd_manifest);
        {
            let lineage = store
                .chunks_lineage
                .get(&chunk.id())
                .expect("a manifest chunk must be tracked");
            assert_eq!(lineage.ref_count, 0, "no physical reference yet");
            assert_eq!(lineage.descends_from_manifest, true);
            assert!(matches!(
                lineage.lineage,
                ChunkDirectLineage::RootFromManifest { .. }
            ));
        }
        assert_eq!(
            false,
            store.physical_chunks_per_chunk_id.contains_key(&chunk.id()),
        );
        assert_eq!(true, store.descends_from_manifest(&chunk.id()));

        // Load it physically. The manifest lineage must not get clobbered.
        store.insert_chunk(&chunk).unwrap();
        {
            let lineage = &store.chunks_lineage[&chunk.id()];
            assert_eq!(lineage.ref_count, 1, "now physically referenced");
            assert_eq!(lineage.descends_from_manifest, true);
            assert!(
                matches!(lineage.lineage, ChunkDirectLineage::RootFromManifest { .. }),
                "physical insertion must keep the manifest lineage",
            );
        }
        assert_store_invariants(&store);

        // GC the physical data away. Because the chunk descends from a manifest, its lineage must
        // stick around so the data stays re-fetchable, unlike a volatile chunk.
        store.gc(&crate::GarbageCollectionOptions::gc_everything());
        assert_eq!(0, store.num_physical_chunks());
        let lineage = store
            .chunks_lineage
            .get(&chunk.id())
            .expect("a manifest lineage must survive GC");
        assert_eq!(lineage.descends_from_manifest, true);
        assert_eq!(lineage.ref_count, 0);
    }

    #[test]
    fn physical_chunks_keep_live_lineage_through_mixed_workload() {
        let mut store = temporal_store(4);
        let mut make_chunk = chunk_factory();

        assert_store_invariants(&store);

        for round in 0..6 {
            // A few tiny chunks that the compactor will happily merge together.
            for _ in 0..5 {
                store.insert_chunk(&make_chunk(1)).unwrap();
                assert_store_invariants(&store);
            }

            // A fat chunk that gets split into several smaller ones.
            store.insert_chunk(&make_chunk(8)).unwrap();
            assert_store_invariants(&store);

            // Drop part of the store, alternating between shallow-friendly and deep deletions.
            store.gc(&crate::GarbageCollectionOptions {
                target: crate::GarbageCollectionTarget::DropAtLeastFraction(0.5),
                time_budget: std::time::Duration::MAX,
                protect_latest: 0,
                protected_time_ranges: Default::default(),
                protected_chunks: Default::default(),
                furthest_from: None,
                perform_deep_deletions: round % 2 == 0,
            });
            assert_store_invariants(&store);
        }

        // Finally drop everything and make sure the bookkeeping maps don't keep growing: every
        // non-manifest lineage and leaky-compaction entry must be reclaimed.
        store.gc(&crate::GarbageCollectionOptions::gc_everything());
        assert_store_invariants(&store);
        assert_eq!(0, store.num_physical_chunks());
        assert!(
            store.chunks_lineage.is_empty(),
            "no volatile lineage should linger once the store is empty, got {} entries",
            store.chunks_lineage.len(),
        );
        assert!(
            store.leaky_compactions.is_empty(),
            "the leaky-compaction tracker should be empty once the store is empty, got {} entries",
            store.leaky_compactions.len(),
        );
        assert!(
            store.dangling_splits.is_empty(),
            "no dangling-split bookkeeping should linger, got {} entries",
            store.dangling_splits.len(),
        );
        assert!(
            store.split_on_ingest.is_empty(),
            "no split-on-ingest bookkeeping should linger, got {} entries",
            store.split_on_ingest.len(),
        );
    }

    // Test that `ChunkIdSetPerTimePerComponentPerTimelinePerEntity` and `ChunkIdSetPerTimePerTimelinePerEntity`
    // are correctly tracked even under tight time budget.
    #[test]
    fn deep_removal_under_tight_budget_keeps_indices_consistent() {
        let mut store = temporal_store(1); // force every row into its own non-root split chunk
        let mut make_chunk = chunk_factory();

        store.insert_chunk(&make_chunk(8)).unwrap();
        let chunks = store.iter_physical_chunks().cloned().collect_vec();
        assert!(
            chunks.len() >= 2,
            "we need several chunks to exercise an early bail",
        );
        assert_store_invariants(&store);

        // A near-zero budget makes the shallow pass bail after the first chunk, while the deep pass
        // clears every virtual entry regardless of the budget. The two indices must not drift
        // apart, otherwise a later GC pass trips the deep-superset-of-shallow assertion.
        store.remove_chunks_deep(
            chunks,
            Some(std::time::Duration::ZERO),
            crate::ChunkDeletionReason::GarbageCollection,
        );

        assert_store_invariants(&store);
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
            .sorted()
            .map(|chunk_id| (*chunk_id, next_chunk_id()))
            .collect();

        let mut lineage_report = Vec::new();
        for chunk_id in store.physical_chunks_per_chunk_id.keys().sorted() {
            lineage_report.push(store.format_lineage(chunk_id));
        }

        let mut lineage_report = lineage_report.join("\n");
        for (chunk_id, redacted_chunk_id) in redacted_chunk_ids {
            lineage_report =
                lineage_report.replace(&chunk_id.to_string(), &redacted_chunk_id.to_string());
        }

        lineage_report
    }

    fn temporal_config(chunk_max_rows: u64) -> ChunkStoreConfig {
        ChunkStoreConfig {
            enable_changelog: false, // irrelevant
            chunk_max_bytes: u64::MAX,
            chunk_max_rows,
            chunk_max_rows_if_unsorted: chunk_max_rows,
        }
    }

    fn temporal_store(chunk_max_rows: u64) -> ChunkStore {
        ChunkStore::new(
            StoreId::recording("app_id", "rec_id"),
            temporal_config(chunk_max_rows),
        )
    }

    /// Builds chunks with deterministic ids, all on the same entity and timeline.
    fn chunk_factory() -> impl FnMut(usize) -> Arc<Chunk> {
        let mut next_chunk_id = next_chunk_id_generator(1);
        let entity_path = EntityPath::from("this/that");
        let timepoint = [(Timeline::new_sequence("frame"), 1)];
        let points = [MyPoint::new(1.0, 1.0)];

        move |num_rows: usize| {
            let mut builder = Chunk::builder_with_id(next_chunk_id(), entity_path.clone());
            for _ in 0..num_rows {
                builder = builder.with_component_batches(
                    RowId::new(),
                    timepoint,
                    [(MyPoints::descriptor_points(), &points as _)],
                );
            }
            Arc::new(builder.build().unwrap())
        }
    }

    /// All temporal chunk ids that currently live in the virtual indices.
    fn virtual_chunk_ids(store: &ChunkStore) -> ahash::HashSet<ChunkId> {
        let mut ids = ahash::HashSet::default();
        for per_timeline in store.temporal_chunk_ids_per_entity.values() {
            for set_per_time in per_timeline.values() {
                ids.extend(set_per_time.per_start_time.values().flatten().copied());
                ids.extend(set_per_time.per_end_time.values().flatten().copied());
            }
        }
        ids
    }

    /// Checks the two invariants that the ref-counted lineage tracking must uphold.
    ///
    /// Every physical chunk must keep a lineage entry with a non-zero ref count, otherwise
    /// `is_root_chunk` silently treats it as a root and reroutes its GC deletion.
    /// Every physical temporal chunk must also live in the virtual indices, which is exactly the
    /// `deep ⊇ shallow` invariant that `remove_chunks_deep` asserts on.
    fn assert_store_invariants(store: &ChunkStore) {
        let virtual_ids = virtual_chunk_ids(store);

        for chunk in store.physical_chunks_per_chunk_id.values() {
            let chunk_id = chunk.id();

            let lineage = store.chunks_lineage.get(&chunk_id);
            assert!(
                lineage.is_some_and(|l| l.ref_count >= 1),
                "physical chunk {chunk_id} lost its live lineage entry, so is_root_chunk would \
                 wrongly treat it as a root. lineage: {lineage:?}",
            );

            if !chunk.is_static() {
                assert!(
                    virtual_ids.contains(&chunk_id),
                    "physical chunk {chunk_id} is missing from the virtual indices, which breaks \
                     the deep-superset-of-shallow GC invariant",
                );
            }
        }
    }
}
