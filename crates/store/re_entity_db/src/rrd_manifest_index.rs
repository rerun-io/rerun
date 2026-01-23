use std::collections::BTreeMap;

use ahash::HashMap;
use arrow::array::RecordBatch;
use nohash_hasher::{IntMap, IntSet};
use re_byte_size::{MemUsageTree, MemUsageTreeCapture};
use re_chunk::{ChunkId, Timeline, TimelineName};
use re_chunk_store::{ChunkStore, ChunkStoreDiff, ChunkStoreEvent};
use re_log_encoding::{CodecResult, RrdManifest, RrdManifestTemporalMapEntry};
use re_log_types::{AbsoluteTimeRange, StoreKind};

use crate::chunk_promise::{ChunkPromise, ChunkPromises};

mod chunk_prioritizer;
mod time_range_merger;

pub use chunk_prioritizer::{ChunkPrefetchOptions, ChunkPrioritizer, PrefetchError};

/// Is the following chunk loaded?
///
/// The order here is used for priority to show the state in the ui (lower is more prioritized)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LoadState {
    /// The chunk is not loaded, nor being loaded.
    #[default]
    Unloaded,

    /// We have requested it.
    ///
    /// TODO(emilk): move this state to [`ChunkPromises`]
    InTransit,

    /// We have the chole chunk in memory.
    Loaded,
}

impl LoadState {
    pub fn is_loaded(&self) -> bool {
        !self.is_unloaded()
    }

    pub fn is_unloaded(&self) -> bool {
        match self {
            Self::Unloaded | Self::InTransit => true,
            Self::Loaded => false,
        }
    }
}

/// Info about a single chunk that we know ahead of loading it.
#[derive(Clone, Debug, Default)]
pub struct ChunkInfo {
    state: LoadState,

    /// Empty for static chunks
    pub temporals: HashMap<TimelineName, TemporalChunkInfo>,
}

impl re_byte_size::SizeBytes for ChunkInfo {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            state: _,
            temporals,
        } = self;
        temporals.heap_size_bytes()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TemporalChunkInfo {
    pub timeline: Timeline,

    /// The time range covered by this entry.
    pub time_range: AbsoluteTimeRange,

    /// The total number of events in the original chunk for this time range.
    ///
    /// This accumulates for all entities and all components, but still accounts for sparness.
    pub num_rows_for_all_entities_all_components: u64,
}

impl re_byte_size::SizeBytes for TemporalChunkInfo {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

/// A secondary index that keeps track of which chunks have been loaded into memory.
///
/// This is constructed from an [`RrdManifest`], which is what the server sends to the client/viewer.
//
// TODO(RR-3383): support multiple manifests per index.
#[derive(Default)]
#[cfg_attr(feature = "testing", derive(Clone))]
pub struct RrdManifestIndex {
    /// The raw manifest.
    ///
    /// This is known ahead-of-time for _some_ data sources.
    manifest: Option<RrdManifest>,

    /// These are the chunks known to exist in the data source (e.g. remote server).
    ///
    /// The chunk store may split large chunks and merge (compact) small ones,
    /// so what's in the chunk store can differ significantly.
    remote_chunks: HashMap<ChunkId, ChunkInfo>,

    chunk_prioritizer: ChunkPrioritizer,

    /// Full time range per timeline
    timelines: BTreeMap<TimelineName, AbsoluteTimeRange>,

    pub entity_tree: crate::EntityTree,
    entity_has_temporal_data_on_timeline: IntMap<re_chunk::EntityPath, IntSet<TimelineName>>,
    entity_has_static_data: IntSet<re_chunk::EntityPath>,

    native_static_map: re_log_encoding::RrdManifestStaticMap,
    native_temporal_map: re_log_encoding::RrdManifestTemporalMap,

    full_uncompressed_size: u64,
}

impl std::fmt::Debug for RrdManifestIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RrdManifestIndex").finish_non_exhaustive()
    }
}

impl RrdManifestIndex {
    pub fn append(&mut self, manifest: RrdManifest) -> CodecResult<()> {
        re_tracing::profile_function!();

        self.native_static_map = manifest.get_static_data_as_a_map()?;
        self.native_temporal_map = manifest.get_temporal_data_as_a_map()?;

        self.update_timeline_stats();
        self.update_entity_tree();
        self.update_entity_temporal_data();
        self.update_entity_static_data();
        self.chunk_prioritizer.on_rrd_manifest(
            &manifest,
            &self.native_static_map,
            &self.native_temporal_map,
        )?;

        for chunk_id in manifest.col_chunk_id()? {
            self.remote_chunks.insert(chunk_id, Default::default());
        }

        for timelines in self.native_temporal_map.values() {
            for (&timeline, comps) in timelines {
                for chunks in comps.values() {
                    for (&chunk_id, entry) in chunks {
                        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
                        chunk_info
                            .temporals
                            .entry(*timeline.name())
                            .and_modify(|info| {
                                info.time_range = entry.time_range.union(info.time_range);
                                info.num_rows_for_all_entities_all_components += entry.num_rows;
                            })
                            .or_insert(TemporalChunkInfo {
                                timeline,
                                time_range: entry.time_range,
                                num_rows_for_all_entities_all_components: entry.num_rows,
                            });
                    }
                }
            }
        }

        if self.manifest.is_some() {
            re_log::warn!(
                "Received a second RRD manifest schema for the same recording. This is not yet supported."
            );
        }

        self.full_uncompressed_size = manifest
            .col_chunk_byte_size_uncompressed_raw()?
            .values()
            .iter()
            .sum();

        self.manifest = Some(manifest);

        Ok(())
    }

    /// Iterate over all chunks in the manifest.
    pub fn remote_chunks(&self) -> impl Iterator<Item = &ChunkInfo> {
        self.remote_chunks.values()
    }

    /// Info about a chunk that is in the manifest
    pub fn remote_chunk_info(&self, chunk_id: &ChunkId) -> Option<&ChunkInfo> {
        self.remote_chunks.get(chunk_id)
    }

    fn update_timeline_stats(&mut self) {
        for timelines in self.native_temporal_map.values() {
            for (timeline, comps) in timelines {
                let mut timeline_range = self
                    .timelines
                    .get(timeline.name())
                    .copied()
                    .unwrap_or(AbsoluteTimeRange::EMPTY);

                for chunks in comps.values() {
                    for entry in chunks.values() {
                        timeline_range = timeline_range.union(entry.time_range);
                    }
                }

                if timeline_range != AbsoluteTimeRange::EMPTY {
                    self.timelines.insert(*timeline.name(), timeline_range);
                }
            }
        }
    }

    fn update_entity_tree(&mut self) {
        for entity in self
            .native_static_map
            .keys()
            .chain(self.native_temporal_map.keys())
        {
            self.entity_tree.on_new_entity(entity);
        }
    }

    fn update_entity_temporal_data(&mut self) {
        for (entity, timelines) in &self.native_temporal_map {
            self.entity_has_temporal_data_on_timeline
                .entry(entity.clone())
                .or_default()
                .extend(timelines.keys().map(|t| *t.name()));
        }
    }

    fn update_entity_static_data(&mut self) {
        for entity in self.native_static_map.keys() {
            self.entity_has_static_data.insert(entity.clone());
        }
    }

    pub fn entity_has_temporal_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.entity_has_temporal_data_on_timeline
            .get(entity)
            .is_some_and(|timelines| timelines.contains(timeline))
    }

    pub fn entity_has_static_data(&self, entity: &re_chunk::EntityPath) -> bool {
        self.entity_has_static_data.contains(entity)
    }

    pub fn entity_has_data_on_timeline(
        &self,
        entity: &re_chunk::EntityPath,
        timeline: &TimelineName,
    ) -> bool {
        self.entity_has_static_data(entity)
            || self.entity_has_temporal_data_on_timeline(entity, timeline)
    }

    /// False for recordings streamed from SDK via proxy
    pub fn has_manifest(&self) -> bool {
        self.manifest.is_some()
    }

    /// The full manifest, if known.
    pub fn manifest(&self) -> Option<&RrdManifest> {
        self.manifest.as_ref()
    }

    pub fn native_temporal_map(&self) -> &re_log_encoding::RrdManifestTemporalMap {
        &self.native_temporal_map
    }

    pub fn mark_as_loaded(&mut self, chunk_id: ChunkId) {
        let chunk_info = self.remote_chunks.entry(chunk_id).or_default();
        chunk_info.state = LoadState::Loaded;
    }

    pub fn on_events(&mut self, store: &ChunkStore, store_events: &[ChunkStoreEvent]) {
        re_tracing::profile_function!();

        if self.manifest.is_none() {
            return;
        }

        for event in store_events {
            match &event.diff {
                ChunkStoreDiff::Addition(add) => {
                    // This is about marking root-level persistent chunks as either loaded or unloaded.
                    //
                    // It doesn't really matter which chunk we pick because `mark_as` will take care of
                    // walking upwards through the lineage tree until one can be found in any case.
                    //
                    // Picking the unprocessed chunk gives us a better chance of not even needing to walk
                    // back the tree at all.
                    self.mark_as(store, &add.chunk_before_processing.id(), LoadState::Loaded);
                }

                ChunkStoreDiff::Deletion(del) => {
                    self.mark_as(store, &del.chunk.id(), LoadState::Unloaded);
                }

                ChunkStoreDiff::VirtualAddition(_) => {}
            }
        }
    }

    fn mark_as(&mut self, store: &ChunkStore, chunk_id: &ChunkId, state: LoadState) {
        let store_kind = store.id().kind();

        if let Some(chunk_info) = self.remote_chunks.get_mut(chunk_id) {
            chunk_info.state = state;
        } else {
            let root_chunk_ids = store.find_root_rrd_manifests(chunk_id);
            if root_chunk_ids.is_empty() {
                warn_when_editing_recording(
                    store_kind,
                    "Added chunk that was not part of the chunk index",
                );
            } else {
                for (chunk_id, _) in root_chunk_ids {
                    if let Some(chunk_info) = self.remote_chunks.get_mut(&chunk_id) {
                        chunk_info.state = state;
                    } else {
                        warn_when_editing_recording(
                            store_kind,
                            "Added chunk that was not part of the chunk index",
                        );
                    }
                }
            }
        }
    }

    /// When do we have data on this timeline?
    pub fn timeline_range(&self, timeline: &TimelineName) -> Option<AbsoluteTimeRange> {
        self.timelines.get(timeline).copied()
    }

    pub fn chunk_prioritizer(&self) -> &ChunkPrioritizer {
        &self.chunk_prioritizer
    }

    pub fn chunk_prioritizer_mut(&mut self) -> &mut ChunkPrioritizer {
        &mut self.chunk_prioritizer
    }

    pub fn chunk_promises(&self) -> &ChunkPromises {
        self.chunk_prioritizer.chunk_promises()
    }

    pub fn chunk_promises_mut(&mut self) -> &mut ChunkPromises {
        self.chunk_prioritizer.chunk_promises_mut()
    }

    /// Find the next candidates for prefetching.
    pub fn prefetch_chunks(
        &mut self,
        store: &ChunkStore,
        options: &ChunkPrefetchOptions,
        load_chunks: &dyn Fn(RecordBatch) -> ChunkPromise,
    ) -> Result<(), PrefetchError> {
        re_tracing::profile_function!();

        if let Some(manifest) = &self.manifest {
            self.chunk_prioritizer.prioritize_and_prefetch(
                store,
                options,
                load_chunks,
                manifest,
                &mut self.remote_chunks,
            )
        } else {
            Err(PrefetchError::NoManifest)
        }
    }

    /// Creates an iterator of time ranges which are loaded on a specific timeline.
    ///
    /// The ranges are guaranteed to be ordered and non-overlapping.
    pub fn loaded_ranges_on_timeline(
        &self,
        timeline: &Timeline,
    ) -> impl Iterator<Item = AbsoluteTimeRange> {
        re_tracing::profile_function!();

        let mut scratch = Vec::new();
        let mut ranges = Vec::new();

        // First we merge ranges for individual components, since chunks' time ranges
        // often have gaps which we don't want to display other components' chunks
        // loaded state in.
        for timelines in self.native_temporal_map.values() {
            let Some(data) = timelines.get(timeline) else {
                continue;
            };

            re_tracing::profile_scope!("timeline", timeline.name().as_str());

            for chunks in data.values() {
                scratch.extend(chunks.iter().filter_map(|(c, range)| {
                    let state = self.remote_chunk_info(c)?.state;

                    Some(time_range_merger::TimeRange {
                        range: range.time_range,
                        loaded: state.is_loaded(),
                    })
                }));

                ranges.extend(time_range_merger::merge_ranges(&scratch));

                scratch.clear();
            }
        }

        time_range_merger::merge_ranges(&ranges)
            .into_iter()
            .filter(|r| r.loaded)
            .map(|r| r.range)
    }

    /// If `component` is some, this returns all unloaded temporal entries for that specific
    /// component on the given timeline.
    ///
    /// If not, this returns all unloaded temporal entries for `entity`'s components and its
    /// descendants' unloaded temporal entries.
    pub fn unloaded_temporal_entries_for(
        &self,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
        component: Option<re_chunk::ComponentIdentifier>,
    ) -> Vec<RrdManifestTemporalMapEntry> {
        re_tracing::profile_function!();

        if let Some(component) = component {
            let Some(per_timeline) = self.native_temporal_map.get(entity) else {
                return Vec::new();
            };

            let Some(per_entity) = per_timeline.get(timeline) else {
                return Vec::new();
            };

            let Some(per_component) = per_entity.get(&component) else {
                return Vec::new();
            };

            per_component
                .iter()
                .filter(|(chunk, _)| self.is_chunk_unloaded(chunk))
                .map(|(_, entry)| *entry)
                .collect()
        } else {
            // If we don't have a specific component we want to include the entity's children
            let mut res = Vec::new();

            if let Some(tree) = self.entity_tree.subtree(entity) {
                tree.visit_children_recursively(|child| {
                    self.unloaded_temporal_entries_for_entity(&mut res, timeline, child);
                });
            } else {
                #[cfg(debug_assertions)]
                re_log::warn_once!(
                    "[DEBUG] Missing entity tree for {entity} while fetching temporal entities"
                );

                self.unloaded_temporal_entries_for_entity(&mut res, timeline, entity);
            }

            res
        }
    }

    /// Fills `ranges` with unloaded temporal entries for this exact entity (descendants aren't included).
    fn unloaded_temporal_entries_for_entity(
        &self,
        ranges: &mut Vec<RrdManifestTemporalMapEntry>,
        timeline: &re_chunk::Timeline,
        entity: &re_chunk::EntityPath,
    ) {
        re_tracing::profile_function!();

        if let Some(entity_ranges_per_timeline) = self.native_temporal_map.get(entity)
            && let Some(entity_ranges) = entity_ranges_per_timeline.get(timeline)
        {
            for (_, entry) in entity_ranges
                .values()
                .flatten()
                .filter(|(chunk, _)| self.is_chunk_unloaded(chunk))
            {
                ranges.push(*entry);
            }
        }
    }

    fn is_chunk_unloaded(&self, chunk_id: &ChunkId) -> bool {
        self.remote_chunks
            .get(chunk_id)
            .is_none_or(|c| c.state.is_unloaded())
    }

    pub fn full_uncompressed_size(&self) -> u64 {
        self.full_uncompressed_size
    }
}

#[track_caller]
fn warn_when_editing_recording(store_kind: StoreKind, warning: &str) {
    match store_kind {
        StoreKind::Recording => {
            if cfg!(debug_assertions) {
                re_log::warn_once!("[DEBUG] {warning}");
            } else {
                re_log::debug_once!("{warning}");
            }
        }
        StoreKind::Blueprint => {
            // We edit blueprint by generating new chunks in the viewer.
        }
    }
}

impl re_byte_size::SizeBytes for RrdManifestIndex {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            chunk_intervals,
            chunk_promises: _, // TODO(emilk)
            entity_has_static_data,
            entity_has_temporal_data_on_timeline,
            entity_tree,
            manifest_row_from_chunk_id,
            manifest,
            native_static_map,
            native_temporal_map,
            remote_chunks,
            static_chunk_ids,
            timelines,
            full_uncompressed_size: _,
        } = self;

        chunk_intervals.heap_size_bytes()
            + entity_has_static_data.heap_size_bytes()
            + entity_has_temporal_data_on_timeline.heap_size_bytes()
            + entity_tree.heap_size_bytes()
            + manifest_row_from_chunk_id.heap_size_bytes()
            + manifest.heap_size_bytes()
            + native_static_map.heap_size_bytes()
            + native_temporal_map.heap_size_bytes()
            + remote_chunks.heap_size_bytes()
            + static_chunk_ids.heap_size_bytes()
            + timelines.heap_size_bytes()
    }
}

impl MemUsageTreeCapture for RrdManifestIndex {
    fn capture_mem_usage_tree(&self) -> MemUsageTree {
        re_tracing::profile_function!();

        use re_byte_size::SizeBytes as _;

        let Self {
            entity_has_static_data,
            entity_has_temporal_data_on_timeline,
            entity_tree,
            manifest,
            native_static_map,
            native_temporal_map,
            remote_chunks,
            chunk_prioritizer,
            timelines,
            full_uncompressed_size: _,
        } = self;

        let mut node = re_byte_size::MemUsageNode::new();
        node.add("chunk_prioritizer", chunk_prioritizer.total_size_bytes());
        node.add(
            "entity_has_static_data",
            entity_has_static_data.total_size_bytes(),
        );
        node.add(
            "entity_has_temporal_data_on_timeline",
            entity_has_temporal_data_on_timeline.total_size_bytes(),
        );
        node.add("entity_tree", entity_tree.total_size_bytes());
        node.add("manifest", manifest.total_size_bytes());
        node.add("native_static_map", native_static_map.total_size_bytes());
        node.add(
            "native_temporal_map",
            native_temporal_map.total_size_bytes(),
        );
        node.add("remote_chunks", remote_chunks.total_size_bytes());
        node.add("timelines", timelines.total_size_bytes());

        node.into_tree()
    }
}
