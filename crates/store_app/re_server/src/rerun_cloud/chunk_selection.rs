use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use nohash_hasher::{IntMap, IntSet};

use re_chunk_index::{ChunkProvider as _, RrdManifest};
use re_chunk_store::{Chunk, ChunkId, ChunkTrackingMode, RangeQuery};
use re_log_types::{AbsoluteTimeRange, EntityPath, TimelineName};
use re_protos::cloud::v1alpha1::ext::{self, ChunkCandidatesDataframe, QueryDatasetRequest};
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

/// Selects the chunks one layer of one segment contributes to a response.
///
/// Applies the request's entity-path, chunk-id, and static/temporal filters. When `samples` has
/// index values for the store's segment, runs one latest-at query per value and deduplicates the
/// result.
pub fn select_chunks(
    store: &LayerStore,
    request: &QueryDatasetRequest,
    samples: Option<&BTreeMap<SegmentId, Vec<re_log_types::TimeInt>>>,
) -> Vec<SelectedChunk> {
    let LayerStore {
        segment_id,
        layer,
        store_slot_id,
        resolved,
    } = store;

    let entity_paths: IntSet<EntityPath> = request.entity_paths.iter().cloned().collect();

    // Build metadata for all relevant chunks (physical + virtual).
    let metadata_vec: Vec<ChunkMetadata> = if let Some(query) = &request.query {
        // RR-4355: per-segment index values pushdown.
        //
        // When the request carries `per_segment_values`, fan out
        // `get_chunks_for_query_results` once per value for this
        // segment with a synthesized latest-at, then dedup. Per
        // the proto contract (`cloud.proto`):
        //   "An empty values list for a segment means no temporal
        //    chunks are returned for that segment (only static
        //    data)."
        // For the empty case we run a single static-only query
        // instead of returning nothing, so static chunks still
        // surface.
        let (chunks, missing_virtual) = if let Some(map) = samples {
            if let Some(values) = map.get(segment_id) {
                let synthesized: Vec<re_log_types::TimeInt> = if values.is_empty() {
                    vec![re_log_types::TimeInt::STATIC]
                } else {
                    values.clone()
                };
                let mut all_chunks: Vec<Arc<Chunk>> = Vec::new();
                let mut all_missing: BTreeSet<ChunkId> = BTreeSet::new();
                let mut seen: BTreeSet<ChunkId> = BTreeSet::new();
                for v in &synthesized {
                    let mut q = query.clone();
                    if let Some(la) = q.latest_at.as_mut() {
                        la.at = *v;
                        la.per_segment_values = Vec::new();
                    }
                    let (cs, missing) = get_chunks_for_query_results(
                        resolved,
                        &entity_paths,
                        request.select_all_entity_paths,
                        &q,
                    );
                    for c in cs {
                        if seen.insert(c.id()) {
                            all_chunks.push(c);
                        }
                    }
                    all_missing.extend(missing);
                }
                for id in &seen {
                    all_missing.remove(id);
                }
                (all_chunks, all_missing.into_iter().collect())
            } else {
                (Vec::new(), Vec::new())
            }
        } else {
            get_chunks_for_query_results(
                resolved,
                &entity_paths,
                request.select_all_entity_paths,
                query,
            )
        };

        let mut metas: Vec<_> = chunks
            .iter()
            .map(|c| ChunkMetadata::from_chunk(c))
            .collect();
        if let ResolvedStore::Lazy(lazy) = resolved {
            for chunk_id in &missing_virtual {
                if let Some(idx) = lazy.chunk_row_index(chunk_id) {
                    metas.push(ChunkMetadata::from_manifest(
                        lazy.manifest(),
                        *chunk_id,
                        idx,
                        lazy.timeline_ranges().get(chunk_id),
                    ));
                }
            }
        }
        metas
    } else {
        match resolved {
            ResolvedStore::Eager(h) => h
                .read()
                .iter_physical_chunks()
                .map(|c| ChunkMetadata::from_chunk(c))
                .collect(),
            ResolvedStore::Lazy(lazy) => lazy
                .manifest()
                .col_chunk_ids()
                .iter()
                .enumerate()
                .map(|(idx, &chunk_id)| {
                    ChunkMetadata::from_manifest(
                        lazy.manifest(),
                        chunk_id,
                        idx,
                        lazy.timeline_ranges().get(&chunk_id),
                    )
                })
                .collect(),
        }
    };

    metadata_vec
        .into_iter()
        .filter(|meta| {
            if !request.select_all_entity_paths && !entity_paths.contains(&meta.entity_path) {
                return false;
            }

            if !request.chunk_ids.is_empty() && !request.chunk_ids.contains(&meta.chunk_id) {
                return false;
            }

            // Filter by static/temporal data
            if request.exclude_static_data && meta.is_static {
                return false;
            }
            if request.exclude_temporal_data && !meta.is_static {
                return false;
            }

            true
        })
        .map(|meta| SelectedChunk {
            segment_id: segment_id.clone(),
            layer: layer.clone(),
            store_slot_id: *store_slot_id,
            meta,
        })
        .collect()
}

/// A `FindChunks` mode's segment scope (`None` is every segment) and, for
/// [`ext::ChunkMode::Samples`] (whose scope is its map's keys), its per-segment sample times.
type ModeScope = (
    Option<BTreeSet<SegmentId>>,
    Option<BTreeMap<SegmentId, Vec<re_log_types::TimeInt>>>,
);

/// Splits a mode into its segment scope and, for Samples, its sample times.
pub fn scope_from_mode(mode: &ext::ChunkMode) -> ModeScope {
    match mode {
        ext::ChunkMode::Static { segments } | ext::ChunkMode::Range { segments, .. } => (
            match segments {
                ext::SegmentSelection::All => None,
                ext::SegmentSelection::Explicit(ids) => Some(ids.clone()),
            },
            None,
        ),
        ext::ChunkMode::Samples { segments, .. } => (
            Some(segments.keys().cloned().collect()),
            Some(
                segments
                    .iter()
                    .map(|(id, values)| (id.clone(), values.iter().copied().collect()))
                    .collect(),
            ),
        ),
    }
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

/// Returns physical chunks and missing virtual chunk IDs for a query.
fn get_chunks_for_query_results(
    resolved: &ResolvedStore,
    entity_paths: &IntSet<EntityPath>,
    select_all_entity_paths: bool,
    query: &ext::Query,
) -> (Vec<Arc<Chunk>>, Vec<ChunkId>) {
    // Contract: a Query with neither `latest_at` nor `range` means "all chunks", regardless of
    // entity filter. This is exercised by the shared `re_redap_tests::query_dataset` "default" test
    // case.
    if query.latest_at.is_none() && query.range.is_none() {
        return match resolved {
            ResolvedStore::Eager(h) => (h.read().iter_physical_chunks().cloned().collect(), vec![]),
            ResolvedStore::Lazy(lazy) => (vec![], lazy.manifest().col_chunk_ids().to_vec()),
        };
    }

    let paths = if select_all_entity_paths {
        resolved.all_entities()
    } else if entity_paths.is_empty() {
        // Per `cloud.proto`: `(select_all_entity_paths=false, entity_paths=[])`
        // is a valid query that selects no entities and yields no results.
        return (Vec::new(), Vec::new());
    } else {
        entity_paths.clone()
    };

    let mut all_chunks: Vec<Arc<Chunk>> = vec![];
    let mut all_missing: BTreeSet<ChunkId> = BTreeSet::new();
    let mut seen_physical: BTreeSet<ChunkId> = BTreeSet::new();

    for entity_path in &paths {
        if let Some(latest_at) = &query.latest_at {
            let latest_at_q = super::latest_at_or_static(latest_at);
            let results = resolved.latest_at_relevant_chunks_for_all_components(
                ChunkTrackingMode::Report,
                &latest_at_q,
                entity_path,
                true,
            );
            for chunk in results.chunks {
                if seen_physical.insert(chunk.id()) {
                    all_chunks.push(chunk);
                }
            }
            all_missing.extend(results.missing_virtual);
        }
        if let Some(range) = &query.range {
            let range_q = RangeQuery::new(range.index, range.index_range);
            let results = resolved.range_relevant_chunks_for_all_components(
                ChunkTrackingMode::Report,
                &range_q,
                entity_path,
                true,
            );
            for chunk in results.chunks {
                if seen_physical.insert(chunk.id()) {
                    all_chunks.push(chunk);
                }
            }
            // Range tightening for virtual chunks. `range_relevant_chunks_for_all_components`
            // post-filters physical chunks against the per-chunk timeline range, but the
            // start-time-indexed scan that produces `missing_virtual` can pull in chunks
            // whose actual time range falls outside the query (the index lookup widens by
            // the longest chunk interval). Without this drop, lazy stores leak those
            // chunks to the client and rows outside the requested range show up in the
            // result set. Latest-at is unaffected because it doesn't fan out via
            // `missing_virtual` here.
            for chunk_id in results.missing_virtual {
                let keep = match resolved {
                    // Eager stores already went through the physical post-filter above.
                    ResolvedStore::Eager(_) => true,
                    ResolvedStore::Lazy(lazy) => match lazy.timeline_ranges().get(&chunk_id) {
                        // No temporal entry => static chunk; let it through (matches the
                        // `chunk.is_static() && include_static` branch of the physical filter).
                        None => true,
                        Some(per_timeline) => per_timeline
                            .get(&range.index)
                            .is_some_and(|time_range| time_range.intersects(range.index_range)),
                    },
                };
                if keep {
                    all_missing.insert(chunk_id);
                }
            }
        }
    }

    // Remove any virtual IDs that turned out to be physical in another entity's result.
    for id in &seen_physical {
        all_missing.remove(id);
    }

    (all_chunks, all_missing.into_iter().collect())
}
