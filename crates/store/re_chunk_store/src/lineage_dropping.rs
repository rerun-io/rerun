use ahash::{HashMap, HashSet};

use re_chunk::ChunkId;

use crate::lineage::TrackedDirectChunkLineage;
use crate::{ChunkDirectLineage, ChunkStore};

// ---

pub(crate) struct LineageDroppingCtx<'a> {
    pub chunks_lineage: &'a mut HashMap<ChunkId, TrackedDirectChunkLineage>,
    pub leaky_compactions: &'a mut HashMap<ChunkId, ChunkId>,
    pub split_on_ingest: &'a mut HashSet<ChunkId>,
    pub dangling_splits: &'a mut HashMap<ChunkId, Vec<ChunkId>>,
}

impl ChunkStore {
    /// Drops one lineage reference on `chunk_id`, cascading removals through the lineage graph.
    ///
    /// Implemented iteratively with an explicit worklist: compaction chains grow one link per
    /// compaction event and can therefore be arbitrarily deep, which would overflow the stack
    /// if this was implemented recursively (RR-5146).
    pub(crate) fn drop_lineage_reference(ctx: &mut LineageDroppingCtx<'_>, chunk_id: &ChunkId) {
        re_tracing::profile_function!();

        enum Op {
            /// Decrement the ref-count, scheduling a `Remove` if it reaches zero.
            DropRef(ChunkId),

            /// Remove the lineage entry and drop one reference on each chunk it references.
            Remove(ChunkId),
        }

        let mut ops = vec![Op::DropRef(*chunk_id)];

        while let Some(op) = ops.pop() {
            match op {
                Op::DropRef(chunk_id) => {
                    let Some(lineage) = ctx.chunks_lineage.get_mut(&chunk_id) else {
                        continue;
                    };

                    lineage.ref_count = lineage.ref_count.saturating_sub(1);

                    if lineage.descends_from_manifest || 0 < lineage.ref_count {
                        continue;
                    }

                    // Only remove splits if all siblings aren't referenced.
                    if let ChunkDirectLineage::SplitFrom(_, siblings) = &lineage.lineage {
                        let siblings = siblings.clone();

                        if siblings.iter().any(|chunk_id| {
                            ctx.chunks_lineage
                                .get(chunk_id)
                                .is_some_and(|l| 0 < l.ref_count)
                        }) {
                            continue;
                        }

                        ops.extend(siblings.iter().map(|chunk_id| Op::Remove(*chunk_id)));
                    }

                    ops.push(Op::Remove(chunk_id));
                }

                Op::Remove(chunk_id) => {
                    if let Some(lineage) = ctx.chunks_lineage.remove(&chunk_id) {
                        ctx.leaky_compactions.remove(&chunk_id);
                        ctx.split_on_ingest.remove(&chunk_id);
                        ctx.dangling_splits.remove(&chunk_id);

                        ops.extend(
                            lineage
                                .iter_referenced_chunks()
                                .map(|chunk_id| Op::DropRef(*chunk_id)),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn lineage_dropping_test(
        chunks_lineage: &mut HashMap<ChunkId, TrackedDirectChunkLineage>,
        chunk_id: &ChunkId,
    ) {
        let mut leaky_compactions = HashMap::default();
        let mut split_on_ingest = HashSet::default();
        let mut dangling_splits = HashMap::default();

        ChunkStore::drop_lineage_reference(
            &mut LineageDroppingCtx {
                chunks_lineage,
                leaky_compactions: &mut leaky_compactions,
                split_on_ingest: &mut split_on_ingest,
                dangling_splits: &mut dangling_splits,
            },
            chunk_id,
        );
    }

    /// Regression test for RR-5146: compaction chains grow one lineage link per compaction
    /// event, so they can get arbitrarily deep. Dropping the final chunk used to unwind the
    /// whole chain recursively, overflowing the stack.
    #[test]
    fn drop_lineage_reference_deep_compaction_chain() {
        const DEPTH: usize = 1_000_000;

        let mut chunks_lineage = HashMap::default();

        let root = ChunkId::new();
        chunks_lineage.insert(
            root,
            TrackedDirectChunkLineage {
                lineage: ChunkDirectLineage::Volatile,
                ref_count: 1,
                descends_from_manifest: false,
            },
        );

        let mut newest = root;
        for _ in 0..DEPTH {
            let chunk_id = ChunkId::new();
            chunks_lineage.insert(
                chunk_id,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::CompactedFrom(vec![newest].into_boxed_slice()),
                    ref_count: 1,
                    descends_from_manifest: false,
                },
            );
            newest = chunk_id;
        }

        lineage_dropping_test(&mut chunks_lineage, &newest);

        assert!(
            chunks_lineage.is_empty(),
            "The whole unreferenced chain should have been removed",
        );
    }

    /// The cascade must stop at entries that descend from a manifest or are still referenced.
    #[test]
    fn drop_lineage_reference_stops_at_manifest_and_referenced() {
        let manifest_root = ChunkId::new();
        let referenced = ChunkId::new();
        let middle = ChunkId::new();
        let top = ChunkId::new();

        let mut chunks_lineage: HashMap<ChunkId, TrackedDirectChunkLineage> = [
            (
                manifest_root,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::RootFromManifest {
                        is_static: false,
                        segment_id: Arc::new("some_segment".into()),
                    },
                    ref_count: 1,
                    descends_from_manifest: true,
                },
            ),
            (
                referenced,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::Volatile,
                    ref_count: 2, // referenced by `middle` and by something external
                    descends_from_manifest: false,
                },
            ),
            (
                middle,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::CompactedFrom(
                        vec![manifest_root, referenced].into_boxed_slice(),
                    ),
                    ref_count: 1,
                    descends_from_manifest: false,
                },
            ),
            (
                top,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::CompactedFrom(vec![middle].into_boxed_slice()),
                    ref_count: 1,
                    descends_from_manifest: false,
                },
            ),
        ]
        .into_iter()
        .collect();

        lineage_dropping_test(&mut chunks_lineage, &top);

        assert!(!chunks_lineage.contains_key(&top));
        assert!(!chunks_lineage.contains_key(&middle));
        assert!(
            chunks_lineage.contains_key(&manifest_root),
            "Manifest-backed entries must never be removed",
        );
        assert_eq!(chunks_lineage[&referenced].ref_count, 1);
    }

    /// Split chunks are only removed once all their siblings are unreferenced.
    #[test]
    fn drop_lineage_reference_split_siblings() {
        let parent = ChunkId::new();
        let split_a = ChunkId::new();
        let split_b = ChunkId::new();

        let make_lineage = |sibling: ChunkId, ref_count: u32| TrackedDirectChunkLineage {
            lineage: ChunkDirectLineage::SplitFrom(parent, vec![sibling].into_boxed_slice()),
            ref_count,
            descends_from_manifest: false,
        };

        let mut chunks_lineage: HashMap<ChunkId, TrackedDirectChunkLineage> = [
            (
                parent,
                TrackedDirectChunkLineage {
                    lineage: ChunkDirectLineage::Volatile,
                    ref_count: 2,
                    descends_from_manifest: false,
                },
            ),
            (split_a, make_lineage(split_b, 1)),
            (split_b, make_lineage(split_a, 1)),
        ]
        .into_iter()
        .collect();

        // Dropping `split_a` while `split_b` is still referenced must keep everything around.
        lineage_dropping_test(&mut chunks_lineage, &split_a);

        assert_eq!(chunks_lineage.len(), 3);
        assert_eq!(chunks_lineage[&split_a].ref_count, 0);

        // Dropping `split_b` releases both splits (and, transitively, the parent).
        lineage_dropping_test(&mut chunks_lineage, &split_b);

        assert!(
            chunks_lineage.is_empty(),
            "Both splits and their parent should have been removed",
        );
    }
}
