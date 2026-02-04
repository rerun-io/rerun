//! Pre-computed sorted chunk cache used for UI drawing.
//!
//! This module provides a secondary index over the manifest's temporal map that:
//! - Pre-sorts chunks by time range for faster iteration.
//! - Aggregates chunks from child entities into their parents.
//!
//! # Why this cache exists
//!
//! The raw manifest data (`RrdManifestTemporalMap`) is organized by entity → timeline → component → chunks.
//! This structure is efficient for lookups by specific component, but inefficient for:
//! - Iterating all chunks under an entity subtree.
//! - Getting chunks sorted by time.
//!
//! This cache pre-computes these operations when the manifest is loaded.

use std::collections::BTreeMap;

use nohash_hasher::IntMap;
use re_chunk::{ChunkId, TimelineName};
use re_log_types::AbsoluteTimeRange;

/// Summary information about a chunk for display/query purposes.
#[derive(Clone)]
pub struct ChunkCountInfo {
    /// The chunk this info is about.
    pub id: ChunkId,

    /// The time range covered by this chunk on the given timeline.
    pub time_range: AbsoluteTimeRange,

    /// Number of rows in this chunk, relevant to the context it's in.
    ///
    /// For the whole entity this is all the rows on the given timeline.
    ///
    /// When this is for a specific component this is the number of rows
    /// for said component on the given timeline.
    pub num_rows: u64,
}

impl re_byte_size::SizeBytes for ChunkCountInfo {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            id: _,
            time_range: _,
            num_rows: _,
        } = self;

        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

/// Sorted chunk information for a single entity on a single timeline.
#[derive(Default, Clone)]
pub(super) struct SortedEntityTemporalChunks {
    /// Chunks for this entity and all its children in the entity tree.
    ///
    /// This includes:
    /// - All chunks from `Self.per_component`.
    /// - All chunks from descendant entities.
    ///
    /// Chunks are unique and sorted by `time_range.min`.
    per_entity: Vec<ChunkCountInfo>,

    /// Chunks organized by component for this specific entity.
    ///
    /// Each component's chunks are unique and sorted by `time_range.min`.
    ///
    /// This does NOT include data from child entities.
    per_component: IntMap<re_chunk::ComponentIdentifier, Vec<ChunkCountInfo>>,
}

impl re_byte_size::SizeBytes for SortedEntityTemporalChunks {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            per_entity,
            per_component,
        } = self;

        per_entity.heap_size_bytes() + per_component.heap_size_bytes()
    }
}

impl SortedEntityTemporalChunks {
    /// Chunks for this entity subtree.
    pub fn per_entity(&self) -> &[ChunkCountInfo] {
        &self.per_entity
    }

    /// Chunks for a specific component on this entity.
    pub fn component_chunks(&self, component: &re_chunk::ComponentIdentifier) -> &[ChunkCountInfo] {
        self.per_component
            .get(component)
            .map_or(&[], |v| v.as_slice())
    }
}

/// Pre-sorted temporal chunk cache organized by timeline and entity.
///
/// This cache is rebuilt whenever the manifest is updated via [`Self::update`].
#[derive(Default, Clone)]
pub(super) struct SortedTemporalChunks {
    per_timeline: BTreeMap<TimelineName, IntMap<re_chunk::EntityPath, SortedEntityTemporalChunks>>,
}

impl re_byte_size::SizeBytes for SortedTemporalChunks {
    fn heap_size_bytes(&self) -> u64 {
        let Self { per_timeline } = self;

        per_timeline.heap_size_bytes()
    }
}

impl SortedTemporalChunks {
    /// Update the cache from the manifest's temporal map and entity tree.
    ///
    /// Should be called when a new rrd manifest is appended.
    pub fn update(
        &mut self,
        entity_tree: &crate::EntityTree,
        native_temporal_map: &re_log_encoding::RrdManifestTemporalMap,
    ) {
        re_tracing::profile_function!();

        // First collect per-component data from the manifest
        for (entity, per_timeline) in native_temporal_map {
            for (timeline, per_component) in per_timeline {
                let sorted_per_entity = self.per_timeline.entry(*timeline.name()).or_default();
                let entity_chunks = sorted_per_entity.entry(entity.clone()).or_default();

                for (component, chunks) in per_component {
                    let component_chunks =
                        entity_chunks.per_component.entry(*component).or_default();
                    component_chunks.extend(chunks.iter().map(|(id, entry)| ChunkCountInfo {
                        id: *id,
                        time_range: entry.time_range,
                        num_rows: entry.num_rows,
                    }));

                    // Dedup chunks
                    component_chunks.sort_by_key(|info| info.id);
                    component_chunks.dedup_by_key(|info| info.id);

                    // Then sort by start time
                    component_chunks.sort_by_key(|info| info.time_range.min);
                }
            }
        }

        /// Bottom-up entity traversal
        fn visit(current: &crate::EntityTree, visitor: &mut impl FnMut(&crate::EntityTree)) {
            for child in current.children.values() {
                visit(child, visitor);
            }
            visitor(current);
        }

        visit(entity_tree, &mut |node: &crate::EntityTree| {
            for per_entity in self.per_timeline.values_mut() {
                // Collect all chunks from direct children which now already includes
                // their descendants and components
                let child_chunks = node
                    .children
                    .values()
                    .filter_map(|v| per_entity.get(&v.path).map(|c| c.per_entity.iter()))
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>();

                let mut entry = per_entity.entry(node.path.clone());
                let chunks = match entry {
                    std::collections::hash_map::Entry::Occupied(ref mut entry) => {
                        let chunks = entry.get_mut();
                        chunks.per_entity.extend(child_chunks);
                        chunks
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(SortedEntityTemporalChunks {
                            per_entity: child_chunks,
                            per_component: Default::default(),
                        })
                    }
                };

                // Collect this entity's own component chunks
                chunks
                    .per_entity
                    .extend(chunks.per_component.values().flatten().cloned());

                // Deduplicate while also merging counts and time ranges.
                //
                // The `native_temporal_map` stores row counts per-component, not for the whole chunk.
                // So if the same chunk appears for multiple components, we need to union their time and
                // sum up their row counts.
                chunks.per_entity.sort_by_key(|info| info.id);
                chunks.per_entity.dedup_by(|a, b| {
                    if a.id == b.id {
                        // Same chunk ID: merge into b
                        b.time_range = b.time_range.union(a.time_range);
                        b.num_rows += a.num_rows;
                        true
                    } else {
                        false
                    }
                });

                // Then sort by time range
                chunks.per_entity.sort_by_key(|info| info.time_range.min);
            }
        });
    }

    /// Get the sorted chunks for an entity on a timeline.
    pub fn get(
        &self,
        timeline: &TimelineName,
        entity: &re_chunk::EntityPath,
    ) -> Option<&SortedEntityTemporalChunks> {
        self.per_timeline.get(timeline)?.get(entity)
    }

    /// Iterate over all component chunk lists on a timeline.
    pub fn iter_all_component_chunks_on_timeline(
        &self,
        timeline: TimelineName,
    ) -> impl Iterator<Item = &[ChunkCountInfo]> {
        self.per_timeline
            .get(&timeline)
            .into_iter()
            .flat_map(|per_entity| {
                per_entity
                    .values()
                    .flat_map(|e| e.per_component.values().map(|v| v.as_slice()))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use re_chunk::EntityPath;
    use re_log_types::TimeInt;

    fn make_entity_tree(paths: &[&EntityPath]) -> crate::EntityTree {
        let mut tree = crate::EntityTree::root();
        for path in paths {
            tree.on_new_entity(path);
        }
        tree
    }

    #[test]
    fn test_empty_cache() {
        let cache = SortedTemporalChunks::default();
        let timeline = TimelineName::new("test");
        let entity = EntityPath::from("/test");

        assert!(cache.get(&timeline, &entity).is_none());
        assert_eq!(
            cache
                .iter_all_component_chunks_on_timeline(timeline)
                .count(),
            0
        );
    }

    #[test]
    fn test_component_chunks_returns_empty_slice_for_missing() {
        let cache = SortedTemporalChunks::default();
        let timeline = TimelineName::new("test");
        let entity = EntityPath::from("/test");

        // When entity doesn't exist, get returns None
        assert!(cache.get(&timeline, &entity).is_none());

        // But if we had an entity with no specific component, it should return empty slice
        let entity_chunks = SortedEntityTemporalChunks::default();
        let component = re_chunk::ComponentIdentifier::new("test:Component");
        assert!(entity_chunks.component_chunks(&component).is_empty());
    }

    #[test]
    fn test_chunks_sorted_by_time() {
        let mut cache = SortedTemporalChunks::default();
        let timeline = TimelineName::new("test");
        let entity = EntityPath::from("/test");
        let component = re_chunk::ComponentIdentifier::new("test:Position");

        // Create a temporal map with chunks in non-sorted order
        let mut temporal_map = re_log_encoding::RrdManifestTemporalMap::default();
        let chunk1 = ChunkId::new();
        let chunk2 = ChunkId::new();
        let chunk3 = ChunkId::new();

        let timeline_obj = re_chunk::Timeline::new_sequence(timeline);

        let mut per_component = IntMap::default();
        let mut chunks = std::collections::BTreeMap::default();
        chunks.insert(
            chunk2,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(200),
                    TimeInt::new_temporal(300),
                ),
                num_rows: 10,
            },
        );
        chunks.insert(
            chunk1,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(100),
                    TimeInt::new_temporal(150),
                ),
                num_rows: 5,
            },
        );
        chunks.insert(
            chunk3,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(50),
                    TimeInt::new_temporal(80),
                ),
                num_rows: 3,
            },
        );
        per_component.insert(component, chunks);

        let mut per_timeline = IntMap::default();
        per_timeline.insert(timeline_obj, per_component);
        temporal_map.insert(entity.clone(), per_timeline);

        let entity_tree = make_entity_tree(&[&entity]);
        cache.update(&entity_tree, &temporal_map);

        // Verify chunks are sorted by time_range.min
        let sorted = cache.get(&timeline, &entity).unwrap();
        let component_chunks = sorted.component_chunks(&component);
        assert_eq!(component_chunks.len(), 3);
        assert!(component_chunks[0].time_range.min < component_chunks[1].time_range.min);
        assert!(component_chunks[1].time_range.min < component_chunks[2].time_range.min);
    }

    #[test]
    fn test_child_chunks_aggregated_to_parent() {
        let mut cache = SortedTemporalChunks::default();
        let timeline = TimelineName::new("test");
        let parent = EntityPath::from("/parent");
        let child = EntityPath::from("/parent/child");
        let component = re_chunk::ComponentIdentifier::new("test:Position");

        let mut temporal_map = re_log_encoding::RrdManifestTemporalMap::default();
        let parent_chunk = ChunkId::new();
        let child_chunk = ChunkId::new();

        let timeline_obj = re_chunk::Timeline::new_sequence(timeline);

        // Add chunk to parent
        let mut parent_per_component = IntMap::default();
        let mut parent_chunks = std::collections::BTreeMap::default();
        parent_chunks.insert(
            parent_chunk,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(100),
                    TimeInt::new_temporal(200),
                ),
                num_rows: 10,
            },
        );
        parent_per_component.insert(component, parent_chunks);

        let mut parent_per_timeline = IntMap::default();
        parent_per_timeline.insert(timeline_obj, parent_per_component);
        temporal_map.insert(parent.clone(), parent_per_timeline);

        // Add chunk to child
        let mut child_per_component = IntMap::default();
        let mut child_chunks = std::collections::BTreeMap::default();
        child_chunks.insert(
            child_chunk,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(50),
                    TimeInt::new_temporal(150),
                ),
                num_rows: 5,
            },
        );
        child_per_component.insert(component, child_chunks);

        let mut child_per_timeline = IntMap::default();
        child_per_timeline.insert(timeline_obj, child_per_component);
        temporal_map.insert(child.clone(), child_per_timeline);

        let entity_tree = make_entity_tree(&[&parent, &child]);
        cache.update(&entity_tree, &temporal_map);

        // Parent's per_entity should include both its own chunk and child's chunk
        let parent_sorted = cache.get(&timeline, &parent).unwrap();
        assert_eq!(parent_sorted.per_entity().len(), 2);

        // Child's per_entity should only include its own chunk
        let child_sorted = cache.get(&timeline, &child).unwrap();
        assert_eq!(child_sorted.per_entity().len(), 1);
    }

    #[test]
    fn test_duplicate_chunks_merged() {
        let mut cache = SortedTemporalChunks::default();
        let timeline = TimelineName::new("test");
        let entity = EntityPath::from("/test");
        let component1 = re_chunk::ComponentIdentifier::new("test:Position");
        let component2 = re_chunk::ComponentIdentifier::new("test:Color");

        let mut temporal_map = re_log_encoding::RrdManifestTemporalMap::default();
        let chunk_id = ChunkId::new();

        let timeline_obj = re_chunk::Timeline::new_sequence(timeline);

        // Same chunk appears in two components with different row counts
        let mut per_component = IntMap::default();

        let mut chunks1 = std::collections::BTreeMap::default();
        chunks1.insert(
            chunk_id,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(100),
                    TimeInt::new_temporal(200),
                ),
                num_rows: 10,
            },
        );
        per_component.insert(component1, chunks1);

        let mut chunks2 = std::collections::BTreeMap::default();
        chunks2.insert(
            chunk_id,
            re_log_encoding::RrdManifestTemporalMapEntry {
                time_range: AbsoluteTimeRange::new(
                    TimeInt::new_temporal(100),
                    TimeInt::new_temporal(200),
                ),
                num_rows: 15,
            },
        );
        per_component.insert(component2, chunks2);

        let mut per_timeline = IntMap::default();
        per_timeline.insert(timeline_obj, per_component);
        temporal_map.insert(entity.clone(), per_timeline);

        let entity_tree = make_entity_tree(&[&entity]);
        cache.update(&entity_tree, &temporal_map);

        // per_entity should have only one entry (deduplicated) with merged row counts
        let sorted = cache.get(&timeline, &entity).unwrap();
        assert_eq!(sorted.per_entity().len(), 1);
        assert_eq!(sorted.per_entity()[0].num_rows, 25); // 10 + 15
    }
}
