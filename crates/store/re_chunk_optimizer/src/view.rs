use std::collections::{BTreeMap, BTreeSet};

use itertools::izip;

use re_chunk::ChunkId;
use re_log_encoding::RawRrdManifest;
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, Timeline};

use crate::Error;

/// The position of a chunk in a [`ChunkIndexView`], i.e. its row in the underlying chunk index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIdx(usize);

/// Everything the chunk index records about one chunk, minus the per-timeline columns.
///
/// The per-timeline data lives in [`EntityView::timeline_sets`].
#[derive(Clone, Debug)]
pub struct ChunkMeta {
    pub chunk_id: ChunkId,
    pub entity_path: EntityPath,
    pub is_static: bool,
    pub num_rows: u64,

    /// Position of the chunk in the RRD file.
    pub rrd_byte_offset: u64,

    /// Size of the chunk in the RRD file.
    pub rrd_byte_size: u64,

    /// In-memory (heap) size of the decoded chunk.
    pub byte_size_uncompressed: u64,
}

/// One chunk's presence on one timeline.
#[derive(Clone, Copy, Debug)]
pub struct ChunkSpan {
    pub chunk: ChunkIdx,

    /// The chunk's time range on this timeline: the union of its per-component ranges.
    ///
    /// The chunk index also stores a global per-timeline range per chunk; the union differs from it
    /// only for rows on which every component is null.
    pub time_range: AbsoluteTimeRange,

    /// Rows with data on this timeline, summed over components.
    ///
    /// A row carrying several components is counted once per component, so this over-counts.
    /// Use it as a score (e.g. to pick a primary timeline), not as a row count.
    pub num_component_rows: u64,
}

/// The temporal chunks of one entity that carry the exact same set of timelines.
///
/// Chunks only ever merge within such a group.
#[derive(Clone, Debug, Default)]
pub struct TimelineSetGroup {
    pub timelines: BTreeSet<Timeline>,

    /// Per timeline: one span per chunk of this group, sorted by range start.
    pub per_timeline: BTreeMap<Timeline, Vec<ChunkSpan>>,
}

impl TimelineSetGroup {
    /// The number of chunks in this group.
    pub fn num_chunks(&self) -> usize {
        self.per_timeline
            .values()
            .next()
            .map_or(0, |spans| spans.len())
    }
}

/// All chunks of one entity, as the chunk index records them.
#[derive(Clone, Debug, Default)]
pub struct EntityView {
    pub static_chunks: Vec<ChunkIdx>,

    /// Temporal chunks, partitioned by their exact timeline set.
    pub timeline_sets: Vec<TimelineSetGroup>,
}

/// A typed, per-entity view over one store's chunk index.
#[derive(Clone, Debug)]
pub struct ChunkIndexView {
    pub store_id: StoreId,

    /// Indexed by [`ChunkIdx`], in chunk index order.
    chunks: Vec<ChunkMeta>,

    pub entities: BTreeMap<EntityPath, EntityView>,

    /// The number of columns of the chunk index itself.
    ///
    /// Recordings whose chunk index exceeds the catalog server's column limit fail registration.
    pub num_columns: usize,
}

impl ChunkIndexView {
    pub fn chunk(&self, idx: ChunkIdx) -> &ChunkMeta {
        &self.chunks[idx.0]
    }

    pub fn num_chunks(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunks(&self) -> impl Iterator<Item = (ChunkIdx, &ChunkMeta)> {
        self.chunks
            .iter()
            .enumerate()
            .map(|(i, meta)| (ChunkIdx(i), meta))
    }

    pub fn try_from_raw(raw: &RawRrdManifest) -> Result<Self, Error> {
        let rows = izip!(
            raw.col_chunk_id()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_ID))?,
            raw.col_chunk_entity_path()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_ENTITY_PATH))?,
            raw.col_chunk_is_static()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_IS_STATIC))?,
            raw.col_chunk_num_rows()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_NUM_ROWS))?,
            raw.col_chunk_byte_offset()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_BYTE_OFFSET))?,
            raw.col_chunk_byte_size()
                .map_err(Error::read_column(RawRrdManifest::FIELD_CHUNK_BYTE_SIZE))?,
            raw.col_chunk_byte_size_uncompressed()
                .map_err(Error::read_column(
                    RawRrdManifest::FIELD_CHUNK_BYTE_SIZE_UNCOMPRESSED,
                ))?,
        );

        let mut chunks: Vec<ChunkMeta> = Vec::with_capacity(raw.data.num_rows());
        let mut idx_by_chunk_id: BTreeMap<ChunkId, ChunkIdx> = BTreeMap::new();
        let mut entities: BTreeMap<EntityPath, EntityView> = BTreeMap::new();

        for (
            i,
            (
                chunk_id,
                entity_path,
                is_static,
                num_rows,
                byte_offset,
                byte_size,
                byte_size_uncompressed,
            ),
        ) in rows.enumerate()
        {
            let idx = ChunkIdx(i);
            idx_by_chunk_id.insert(chunk_id, idx);
            if is_static {
                entities
                    .entry(entity_path.clone())
                    .or_default()
                    .static_chunks
                    .push(idx);
            }
            chunks.push(ChunkMeta {
                chunk_id,
                entity_path,
                is_static,
                num_rows,
                rrd_byte_offset: byte_offset,
                rrd_byte_size: byte_size,
                byte_size_uncompressed,
            });
        }

        // The temporal map iterates in an unspecified order; everything below lands in `BTreeMap`s
        // so the view comes out deterministic.
        let temporal = raw.calc_temporal_map().map_err(Error::temporal_map)?;
        #[expect(clippy::iter_over_hash_type)]
        for (entity_path, per_timeline) in temporal {
            // Per chunk of this entity: its timelines, with the component ranges unioned and the
            // component row counts summed.
            let mut per_chunk: BTreeMap<ChunkIdx, BTreeMap<Timeline, (AbsoluteTimeRange, u64)>> =
                BTreeMap::new();

            #[expect(clippy::iter_over_hash_type)]
            for (timeline, per_component) in per_timeline {
                for per_chunk_entries in per_component.into_values() {
                    for (chunk_id, entry) in per_chunk_entries {
                        let idx = *idx_by_chunk_id.get(&chunk_id).ok_or_else(|| {
                            Error::UnknownChunkId {
                                chunk_id,
                                entity_path: entity_path.clone(),
                            }
                        })?;

                        per_chunk
                            .entry(idx)
                            .or_default()
                            .entry(timeline)
                            .and_modify(|(range, num_rows)| {
                                *range = range.union(entry.time_range);
                                *num_rows += entry.num_rows;
                            })
                            .or_insert((entry.time_range, entry.num_rows));
                    }
                }
            }

            // Partition the entity's chunks by their exact timeline set.
            let mut groups: BTreeMap<BTreeSet<Timeline>, BTreeMap<Timeline, Vec<ChunkSpan>>> =
                BTreeMap::new();
            for (idx, chunk_timelines) in per_chunk {
                let set: BTreeSet<Timeline> = chunk_timelines.keys().copied().collect();
                let group = groups.entry(set).or_default();
                for (timeline, (time_range, num_component_rows)) in chunk_timelines {
                    group.entry(timeline).or_default().push(ChunkSpan {
                        chunk: idx,
                        time_range,
                        num_component_rows,
                    });
                }
            }

            let timeline_sets = groups
                .into_iter()
                .map(|(timelines, mut per_timeline)| {
                    for spans in per_timeline.values_mut() {
                        spans.sort_by_key(|span| {
                            (span.time_range.min(), span.time_range.max(), span.chunk)
                        });
                    }
                    TimelineSetGroup {
                        timelines,
                        per_timeline,
                    }
                })
                .collect();

            entities.entry(entity_path).or_default().timeline_sets = timeline_sets;
        }

        Ok(Self {
            store_id: raw.store_id.clone(),
            chunks,
            entities,
            num_columns: raw.data.num_columns(),
        })
    }
}

#[cfg(test)]
mod tests {
    use re_chunk::{Chunk, ChunkId, RowId};
    use re_log_encoding::RawRrdManifest;
    use re_log_types::example_components::{MyColor, MyPoints};
    use re_log_types::{EntityPath, StoreId, StoreKind, TimePoint, Timeline};
    use re_types_core::ComponentBatch as _;

    use super::ChunkIndexView;

    #[test]
    fn view_construction() -> anyhow::Result<()> {
        let frame = Timeline::new_sequence("frame");

        let static_chunk = Chunk::builder_with_id(ChunkId::from_u128(1), "static_entity")
            .with_serialized_batches(
                RowId::from_u128(1 << 32),
                TimePoint::default(),
                [MyColor::from_iter(0..=0).try_serialized(MyPoints::descriptor_colors())?],
            )
            .build()?;

        let mut temporal = Chunk::builder_with_id(ChunkId::from_u128(2), "temporal");
        for (i, time) in [0_i64, 10].into_iter().enumerate() {
            temporal = temporal.with_serialized_batches(
                RowId::from_u128((2 << 32) + i as u128 + 1),
                [(frame, time)],
                [MyColor::from_iter(0..=0).try_serialized(MyPoints::descriptor_colors())?],
            );
        }
        let temporal = temporal.build()?;

        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let chunk_index =
            RawRrdManifest::build_in_memory_from_chunks(store_id, [static_chunk, temporal].iter())?;
        let view = ChunkIndexView::try_from_raw(&chunk_index)?;

        assert_eq!(view.num_chunks(), 2);
        assert_eq!(view.entities.len(), 2);

        let static_entity = &view.entities[&EntityPath::from("static_entity")];
        assert_eq!(static_entity.static_chunks.len(), 1);
        assert!(static_entity.timeline_sets.is_empty());
        assert!(view.chunk(static_entity.static_chunks[0]).is_static);

        let temporal = &view.entities[&EntityPath::from("temporal")];
        assert!(temporal.static_chunks.is_empty());
        assert_eq!(temporal.timeline_sets.len(), 1);
        let group = &temporal.timeline_sets[0];
        assert_eq!(group.timelines.len(), 1);
        let spans = &group.per_timeline[&frame];
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].time_range.min().as_i64(), 0);
        assert_eq!(spans[0].time_range.max().as_i64(), 10);
        assert_eq!(view.chunk(spans[0].chunk).num_rows, 2);

        Ok(())
    }
}
