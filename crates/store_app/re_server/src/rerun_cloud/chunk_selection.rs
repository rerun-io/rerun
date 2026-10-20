use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use nohash_hasher::{IntMap, IntSet};

use re_chunk_index::{ChunkProvider as _, RrdManifest};
use re_chunk_store::{Chunk, ChunkId, ChunkTrackingMode, LatestAtQuery, RangeQuery};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimeInt, TimelineName};
use re_protos::cloud::v1alpha1::ext::{self, ChunkCandidatesDataframe};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_types_core::LayerName;

use crate::store::{ChunkKey, ResolvedStore, StoreSlotId};

/// Metadata for a single chunk, extractable from either a physical `Chunk` or a manifest.
pub struct ChunkMetadata {
    pub chunk_id: ChunkId,
    pub entity_path: EntityPath,
    pub is_static: bool,
    pub byte_size: u64,
    pub timelines: IntMap<TimelineName, AbsoluteTimeRange>,
}

impl ChunkMetadata {
    fn from_chunk(chunk: &Chunk) -> Self {
        let timelines = chunk
            .timelines()
            .values()
            .map(|col| (*col.timeline().name(), col.time_range()))
            .collect();
        Self {
            chunk_id: chunk.id(),
            entity_path: chunk.entity_path().clone(),
            is_static: chunk.is_static(),
            byte_size: re_byte_size::SizeBytes::total_size_bytes(chunk),
            timelines,
        }
    }

    fn from_manifest(
        manifest: &RrdManifest,
        chunk_id: ChunkId,
        row_idx: usize,
        chunk_timelines: Option<&IntMap<TimelineName, AbsoluteTimeRange>>,
    ) -> Self {
        Self {
            chunk_id,
            entity_path: EntityPath::from(manifest.col_chunk_entity_path().value(row_idx)),
            is_static: manifest.col_chunk_is_static().value(row_idx),
            byte_size: manifest.col_chunk_byte_size_uncompressed()[row_idx],
            timelines: chunk_timelines.cloned().unwrap_or_default(),
        }
    }
}

/// The chunk store holding one layer of one segment.
pub struct LayerStore {
    pub segment_id: SegmentId,
    pub layer: LayerName,

    /// The store's slot in the server's store pool. Chunk keys carry it so `FetchChunks` can
    /// locate the store.
    pub store_slot_id: StoreSlotId,

    pub resolved: ResolvedStore,
}

/// A chunk chosen by [`select_chunks`], with the segment, layer, and store slot its response row
/// needs.
pub struct SelectedChunk {
    pub segment_id: SegmentId,
    pub layer: LayerName,
    pub store_slot_id: StoreSlotId,
    pub meta: ChunkMetadata,
}

/// The entities a request selects chunks for.
///
/// Component filtering happens client-side; [`select_chunks`] only narrows candidates by entity
/// path.
pub enum EntityScope {
    /// Every entity in the store.
    All,

    /// Only the listed entities.
    Explicit(IntSet<EntityPath>),
}

impl EntityScope {
    pub fn new(contents: &ext::ContentSelection) -> Self {
        match contents {
            ext::ContentSelection::All => Self::All,
            ext::ContentSelection::Explicit(entities) => Self::Explicit(
                entities
                    .iter()
                    .map(|entity| entity.entity_path.clone())
                    .collect(),
            ),
        }
    }

    fn contains(&self, entity_path: &EntityPath) -> bool {
        match self {
            Self::All => true,
            Self::Explicit(entity_paths) => entity_paths.contains(entity_path),
        }
    }
}

/// Selects the chunks one layer of one segment contributes to a response.
///
/// `mode == None` scans every chunk on no timeline, for the `QueryDataset` full scan of released
/// SDKs.
// TODO(RR-5817): drop the `None` case with the `QueryDataset` compatibility code.
pub fn select_chunks(
    store: &LayerStore,
    entities: &EntityScope,
    mode: Option<&ext::ChunkMode>,
) -> tonic::Result<Vec<SelectedChunk>> {
    let LayerStore {
        segment_id,
        layer,
        store_slot_id,
        resolved,
    } = store;

    let (chunks, missing_virtual) = match mode {
        None => full_scan_results(resolved),
        Some(ext::ChunkMode::Static { .. }) => {
            let paths = query_paths(resolved, entities);
            let mut results = QueryResults::default();
            add_latest_at_results(&mut results, resolved, &paths, &LatestAtQuery::new_static());
            results.finish()
        }
        Some(ext::ChunkMode::Range { range, .. }) => {
            let include_latest_at = range.fill_mode.includes_latest_at("range.fill_mode")?;
            match (range.start_inclusive, range.end_inclusive) {
                (None, None) => full_scan_results(resolved),
                (start, end) => {
                    let paths = query_paths(resolved, entities);
                    let range_query = RangeQuery::new(
                        range.timeline,
                        AbsoluteTimeRange {
                            min: start.unwrap_or(TimeInt::MIN),
                            max: end.unwrap_or(TimeInt::MAX),
                        },
                    );
                    let mut results = QueryResults::default();
                    if include_latest_at && let Some(start) = start {
                        add_latest_at_results(
                            &mut results,
                            resolved,
                            &paths,
                            &LatestAtQuery::new(range.timeline, start),
                        );
                    }
                    add_range_results(&mut results, resolved, &paths, &range_query);
                    results.finish()
                }
            }
        }
        Some(ext::ChunkMode::Samples {
            timeline,
            segments,
            fill_mode,
        }) => {
            let include_latest_at = fill_mode.includes_latest_at("samples.fill_mode")?;
            let Some(values) = segments.get(segment_id) else {
                return Ok(Vec::new());
            };
            let paths = query_paths(resolved, entities);
            let mut results = QueryResults::default();
            for &value in values {
                if include_latest_at {
                    add_latest_at_results(
                        &mut results,
                        resolved,
                        &paths,
                        &LatestAtQuery::new(*timeline, value),
                    );
                } else {
                    add_range_results(
                        &mut results,
                        resolved,
                        &paths,
                        &RangeQuery::new(*timeline, AbsoluteTimeRange::new(value, value)),
                    );
                }
            }
            results.finish()
        }
    };

    let mut metadata_vec: Vec<_> = chunks
        .iter()
        .map(|c| ChunkMetadata::from_chunk(c))
        .collect();
    if let ResolvedStore::Lazy(lazy) = resolved {
        for chunk_id in &missing_virtual {
            if let Some(idx) = lazy.chunk_row_index(chunk_id) {
                metadata_vec.push(ChunkMetadata::from_manifest(
                    lazy.manifest(),
                    *chunk_id,
                    idx,
                    lazy.timeline_ranges().get(chunk_id),
                ));
            }
        }
    }

    Ok(metadata_vec
        .into_iter()
        .filter(|meta| entities.contains(&meta.entity_path))
        .map(|meta| SelectedChunk {
            segment_id: segment_id.clone(),
            layer: layer.clone(),
            store_slot_id: *store_slot_id,
            meta,
        })
        .collect())
}

/// Encodes one store's `FindChunks` candidates as a [`ChunkCandidatesDataframe`] batch.
///
/// `chunk_timeline_start` is the chunk's minimum on `timeline`, or null if it is static or lacks
/// that timeline. Both direct-URL columns are always null: the OSS server returns no direct URLs.
pub fn encode_candidates(
    chunks: &[SelectedChunk],
    timeline: Option<&TimelineName>,
) -> tonic::Result<RecordBatch> {
    let num_chunks = chunks.len();

    let mut chunk_ids = Vec::with_capacity(num_chunks);
    let mut chunk_segment_ids = Vec::with_capacity(num_chunks);
    let mut chunk_layer_names = Vec::with_capacity(num_chunks);
    let mut chunk_keys = Vec::with_capacity(num_chunks);
    let mut chunk_entity_paths = Vec::with_capacity(num_chunks);
    let mut chunk_is_static = Vec::with_capacity(num_chunks);
    let mut chunk_byte_lens = Vec::with_capacity(num_chunks);
    let mut chunk_byte_sizes_uncompressed = Vec::with_capacity(num_chunks);
    let mut chunk_timeline_starts = Vec::with_capacity(num_chunks);

    for SelectedChunk {
        segment_id,
        layer,
        store_slot_id,
        meta,
    } in chunks
    {
        chunk_ids.push(meta.chunk_id);
        chunk_segment_ids.push(segment_id.clone());
        chunk_layer_names.push(layer.clone());
        chunk_keys.push(
            ChunkKey {
                chunk_id: meta.chunk_id,
                store_slot_id: *store_slot_id,
            }
            .encode()?,
        );
        chunk_entity_paths.push(meta.entity_path.clone());
        chunk_is_static.push(meta.is_static);
        chunk_byte_lens.push(meta.byte_size);
        // OSS server stores decoded data, so compressed == uncompressed.
        chunk_byte_sizes_uncompressed.push(Some(meta.byte_size));

        let start = if meta.is_static {
            None
        } else {
            timeline.and_then(|timeline| {
                meta.timelines
                    .get(timeline)
                    .map(|range| range.min().as_i64())
            })
        };
        chunk_timeline_starts.push(start);
    }

    let chunk_key_refs = chunk_keys.iter().map(Vec::as_slice).collect();

    ChunkCandidatesDataframe::new(
        chunk_ids,
        chunk_segment_ids,
        chunk_layer_names,
        chunk_key_refs,
        chunk_entity_paths,
        chunk_is_static,
        chunk_byte_lens,
        chunk_byte_sizes_uncompressed,
        chunk_timeline_starts,
        vec![None; num_chunks],
        vec![None; num_chunks],
    )
    .map_err(|err| {
        tonic::Status::internal(format!(
            "Failed to build the FindChunks dataframe for {num_chunks} candidates: {err:#}"
        ))
    })?
    .into_record_batch()
    .map_err(|err| {
        tonic::Status::internal(format!(
            "Failed to encode the FindChunks dataframe for {num_chunks} candidates: {err:#}"
        ))
    })
}

fn full_scan_results(resolved: &ResolvedStore) -> (Vec<Arc<Chunk>>, Vec<ChunkId>) {
    match resolved {
        ResolvedStore::Eager(handle) => (
            handle.read().iter_physical_chunks().cloned().collect(),
            Vec::new(),
        ),
        ResolvedStore::Lazy(lazy) => (Vec::new(), lazy.manifest().col_chunk_ids().to_vec()),
    }
}

fn query_paths(resolved: &ResolvedStore, entities: &EntityScope) -> IntSet<EntityPath> {
    match entities {
        EntityScope::All => resolved.all_entities(),
        EntityScope::Explicit(entity_paths) => entity_paths.clone(),
    }
}

#[derive(Default)]
struct QueryResults {
    chunks: Vec<Arc<Chunk>>,
    seen_physical: BTreeSet<ChunkId>,
    missing_virtual: BTreeSet<ChunkId>,
}

impl QueryResults {
    fn extend(
        &mut self,
        chunks: impl IntoIterator<Item = Arc<Chunk>>,
        missing_virtual: impl IntoIterator<Item = ChunkId>,
    ) {
        for chunk in chunks {
            if self.seen_physical.insert(chunk.id()) {
                self.chunks.push(chunk);
            }
        }
        self.missing_virtual.extend(missing_virtual);
    }

    fn finish(mut self) -> (Vec<Arc<Chunk>>, Vec<ChunkId>) {
        for id in self.seen_physical {
            self.missing_virtual.remove(&id);
        }
        (self.chunks, self.missing_virtual.into_iter().collect())
    }
}

fn add_latest_at_results(
    output: &mut QueryResults,
    resolved: &ResolvedStore,
    paths: &IntSet<EntityPath>,
    query: &LatestAtQuery,
) {
    for entity_path in paths {
        let results = resolved.latest_at_relevant_chunks_for_all_components(
            ChunkTrackingMode::Report,
            query,
            entity_path,
            true,
        );
        output.extend(results.chunks, results.missing_virtual);
    }
}

fn add_range_results(
    output: &mut QueryResults,
    resolved: &ResolvedStore,
    paths: &IntSet<EntityPath>,
    query: &RangeQuery,
) {
    for entity_path in paths {
        let results = resolved.range_relevant_chunks_for_all_components(
            ChunkTrackingMode::Report,
            query,
            entity_path,
            true,
        );
        // The lazy index widens its scan by the longest chunk interval. Tighten virtual results
        // against their manifest ranges to match the physical post-filter.
        let missing_virtual =
            results
                .missing_virtual
                .into_iter()
                .filter(|chunk_id| match resolved {
                    ResolvedStore::Eager(_) => true,
                    ResolvedStore::Lazy(lazy) => match lazy.timeline_ranges().get(chunk_id) {
                        None => true,
                        Some(per_timeline) => per_timeline
                            .get(query.timeline())
                            .is_some_and(|time_range| time_range.intersects(query.range())),
                    },
                });
        output.extend(results.chunks, missing_virtual);
    }
}
