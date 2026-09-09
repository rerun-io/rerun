use std::collections::{BTreeMap, BTreeSet};

use itertools::{Either, izip};

use re_chunk::external::arrow::array::BooleanArray;
use re_chunk::{ArrowArray as _, ChunkId, ComponentIdentifier, ComponentType};
use re_log_encoding::RawRrdManifest;
use re_log_types::{AbsoluteTimeRange, EntityPath, StoreId, Timeline};

use crate::Error;

/// The position of a chunk in a [`ChunkIndexView`], i.e. its row in the underlying chunk index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkIdx(usize);

impl ChunkIdx {
    pub fn as_usize(self) -> usize {
        self.0
    }
}

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

    /// Uncompressed size of the chunk. What this measures depends on the source: the length of
    /// the uncompressed Arrow IPC stream on the file path (which charges each chunk a
    /// schema-dependent framing constant), and the decoded heap size on the in-memory path.
    pub byte_size_uncompressed: u64,

    /// Component columns with data in this chunk — a non-null range on some timeline, or the
    /// static flag — with the type each carries when it has one. An all-null temporal column is
    /// absent.
    pub components: BTreeMap<ComponentIdentifier, Option<ComponentType>>,
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
            raw.col_chunk_id_iter()
                .map_err(|err| Error::read_column(RawRrdManifest::COLUMN_CHUNK_ID.name, err))?,
            raw.col_chunk_entity_path_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_ENTITY_PATH.name,
                    err
                ))?,
            raw.col_chunk_is_static_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_IS_STATIC.name,
                    err
                ))?,
            raw.col_chunk_num_rows_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_NUM_ROWS.name,
                    err
                ))?,
            raw.col_chunk_byte_offset_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_BYTE_OFFSET.name,
                    err
                ))?,
            raw.col_chunk_byte_size_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_BYTE_SIZE.name,
                    err
                ))?,
            raw.col_chunk_byte_size_uncompressed_iter()
                .map_err(|err| Error::read_column(
                    RawRrdManifest::COLUMN_CHUNK_BYTE_SIZE_UNCOMPRESSED.name,
                    err
                ))?,
        );

        let mut components = components_per_chunk(raw)?;

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
                components: std::mem::take(&mut components[i]),
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
                        let idx = *idx_by_chunk_id
                            .get(&chunk_id)
                            .ok_or_else(|| Error::unknown_chunk_id(chunk_id, &entity_path))?;

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

/// Per chunk index row: the component columns with data.
///
/// Two descriptors that differ only by type produce two index fields with the same name, so this
/// iterates the fields and never looks one up by name.
fn components_per_chunk(
    raw: &RawRrdManifest,
) -> Result<Vec<BTreeMap<ComponentIdentifier, Option<ComponentType>>>, Error> {
    let num_rows = raw.data.num_rows();
    let mut components = vec![BTreeMap::new(); num_rows];

    let schema = raw.data.schema();
    for (field, column) in std::iter::zip(schema.fields(), raw.data.columns()) {
        let Some(component) = field
            .metadata()
            .get(re_types_core::FIELD_METADATA_KEY_COMPONENT)
        else {
            continue;
        };
        let component = ComponentIdentifier::try_new(component).map_err(|_err| {
            Error::malformed_component_column(field.name(), "empty component identifier")
        })?;
        let component_type = field
            .metadata()
            .get(re_types_core::FIELD_METADATA_KEY_COMPONENT_TYPE)
            .map(ComponentType::try_new)
            .transpose()
            .map_err(|_err| {
                Error::malformed_component_column(field.name(), "empty component type")
            })?;

        // Only the rows with data are visited: for a `:start` column those are its valid rows, for a
        // `:has_static_data` column its set bits. Both come straight off the buffers, so the walk
        // is proportional to the number of (chunk, component) presences, not to fields × rows.
        let rows_with_data: Either<_, _> = if field.name().ends_with(":start") {
            Either::Left(match column.nulls() {
                Some(nulls) => Either::Left(nulls.valid_indices()),
                None => Either::Right(0..num_rows),
            })
        } else if field.name().ends_with(":has_static_data") {
            let flags = column
                .as_any()
                .downcast_ref::<BooleanArray>()
                .ok_or_else(|| {
                    Error::malformed_component_column(field.name(), "expected a boolean column")
                })?;
            Either::Right(flags.values().set_indices())
        } else {
            continue;
        };

        for i in rows_with_data {
            // A chunk holds one column per identifier, so at most one variant has data here;
            // should two ever claim it, keep the typed one.
            components[i]
                .entry(component)
                .and_modify(|existing: &mut Option<ComponentType>| {
                    if existing.is_none() {
                        *existing = component_type;
                    }
                })
                .or_insert(component_type);
        }
    }

    Ok(components)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use re_chunk::{Chunk, ChunkId, RowId};
    use re_log_encoding::RawRrdManifest;
    use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
    use re_log_types::{EntityPath, StoreId, StoreKind, TimePoint, Timeline};
    use re_types_core::{Component as _, ComponentBatch as _, ComponentDescriptor};

    use super::ChunkIndexView;

    /// The same identifier logged typed under an archetype on one entity and untyped on another
    /// gives the index two same-named column sets; the view reads both.
    ///
    /// Background info in RR-5622.
    #[test]
    fn same_identifier_two_descriptors() -> anyhow::Result<()> {
        let frame = Timeline::new_sequence("frame");
        let typed = Chunk::builder_with_id(ChunkId::from_u128(1), "real")
            .with_serialized_batches(
                RowId::from_u128(1 << 32),
                [(frame, 0_i64)],
                [MyPoint::from_iter(0..2).try_serialized(MyPoints::descriptor_points())?],
            )
            .build()?;
        let untyped_descriptor = ComponentDescriptor {
            archetype: None,
            component: MyPoints::descriptor_points().component,
            component_type: None,
        };
        let untyped = Chunk::builder_with_id(ChunkId::from_u128(2), "fake")
            .with_serialized_batches(
                RowId::from_u128(2 << 32),
                [(frame, 0_i64)],
                [
                    MyPoint::from_iter(0..2).try_serialized(untyped_descriptor)?,
                    MyColor::from_iter(0..2).try_serialized(MyPoints::descriptor_colors())?,
                ],
            )
            .build()?;

        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let raw = RawRrdManifest::build_in_memory_from_chunks(store_id, [typed, untyped].iter())?;

        // Both variants are seen, per chunk and per entity.
        let view = ChunkIndexView::try_from_raw(&raw)?;
        let columns = |id: u128| {
            view.chunks()
                .find(|(_, meta)| meta.chunk_id == ChunkId::from_u128(id))
                .map(|(_, meta)| meta.components.keys().copied().collect::<Vec<_>>())
                .unwrap()
        };
        assert_eq!(columns(1), vec![MyPoints::descriptor_points().component]);
        assert_eq!(
            columns(2),
            vec![
                MyPoints::descriptor_colors().component,
                MyPoints::descriptor_points().component,
            ]
        );
        assert_eq!(view.entities.len(), 2);
        for entity in ["real", "fake"] {
            let entity = &view.entities[&EntityPath::from(entity)];
            assert_eq!(entity.timeline_sets.len(), 1);
            assert_eq!(entity.timeline_sets[0].per_timeline[&frame].len(), 1);
        }

        Ok(())
    }

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

        // A typed column next to an untyped one, on the same rows.
        let untyped_descriptor = ComponentDescriptor {
            archetype: None,
            component: "custom".into(),
            component_type: None,
        };
        let mixed = Chunk::builder_with_id(ChunkId::from_u128(3), "temporal")
            .with_serialized_batches(
                RowId::from_u128(3 << 32),
                [(frame, 20_i64)],
                [
                    MyPoint::from_iter(0..2).try_serialized(MyPoints::descriptor_points())?,
                    MyPoint::from_iter(0..2).try_serialized(untyped_descriptor.clone())?,
                ],
            )
            .build()?;

        let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
        let chunk_index = RawRrdManifest::build_in_memory_from_chunks(
            store_id,
            [static_chunk, temporal, mixed].iter(),
        )?;
        let view = ChunkIndexView::try_from_raw(&chunk_index)?;

        assert_eq!(view.num_chunks(), 3);
        assert_eq!(view.entities.len(), 2);

        let static_entity = &view.entities[&EntityPath::from("static_entity")];
        assert_eq!(static_entity.static_chunks.len(), 1);
        assert!(static_entity.timeline_sets.is_empty());
        let static_meta = view.chunk(static_entity.static_chunks[0]);
        assert!(static_meta.is_static);
        assert_eq!(
            static_meta.components,
            BTreeMap::from([(
                MyPoints::descriptor_colors().component,
                Some(MyColor::name())
            )])
        );

        let temporal = &view.entities[&EntityPath::from("temporal")];
        assert!(temporal.static_chunks.is_empty());
        assert_eq!(temporal.timeline_sets.len(), 1);
        let group = &temporal.timeline_sets[0];
        assert_eq!(group.timelines.len(), 1);
        let spans = &group.per_timeline[&frame];
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].time_range.min().as_i64(), 0);
        assert_eq!(spans[0].time_range.max().as_i64(), 10);
        let colors_meta = view.chunk(spans[0].chunk);
        assert_eq!(colors_meta.num_rows, 2);
        assert_eq!(
            colors_meta.components,
            BTreeMap::from([(
                MyPoints::descriptor_colors().component,
                Some(MyColor::name())
            )])
        );

        // The typed column records its type, the untyped one records `None`; neither chunk sees
        // the other's columns.
        let mixed_meta = view.chunk(spans[1].chunk);
        assert_eq!(mixed_meta.chunk_id, ChunkId::from_u128(3));
        assert_eq!(
            mixed_meta.components,
            BTreeMap::from([
                (
                    MyPoints::descriptor_points().component,
                    Some(MyPoint::name())
                ),
                (untyped_descriptor.component, None),
            ])
        );

        Ok(())
    }
}
