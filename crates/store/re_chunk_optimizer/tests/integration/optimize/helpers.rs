//! Chunk builders, providers and settings shared by the test modules.

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use futures::executor::block_on;
use futures::{Stream, TryStreamExt as _};

use re_byte_size::SizeBytes as _;
use re_chunk::{ArrowArray as _, Chunk, ChunkId, RowId};
use re_chunk_index::InMemoryChunkProvider;
use re_chunk_optimizer::{
    ColumnSelector, Error, MergeSplitSettings, OptimizationSettings, OwnChunkRule,
};
use re_log_encoding::{EncodingOptions, RrdChunkProvider};
use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_log_types::{StoreId, StoreKind, Timeline};
use re_types_core::{Component as _, ComponentBatch as _, ComponentIdentifier};

pub fn temporal_point_chunk(
    id: u128,
    entity: &str,
    times: &[i64],
    points_per_row: u32,
) -> Arc<Chunk> {
    let frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &time) in times.iter().enumerate() {
        builder = builder.with_serialized_batches(
            RowId::from_u128((id << 32) + i as u128 + 1),
            [(frame, time)],
            [MyPoint::from_iter(0..points_per_row)
                .try_serialized(MyPoints::descriptor_points())
                .unwrap()],
        );
    }
    Arc::new(builder.build().unwrap())
}

pub fn temporal_color_chunk(
    id: u128,
    entity: &str,
    times: &[i64],
    colors_per_row: u32,
) -> Arc<Chunk> {
    let frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &time) in times.iter().enumerate() {
        builder = builder.with_serialized_batches(
            RowId::from_u128((id << 32) + i as u128 + 1),
            [(frame, time)],
            [MyColor::from_iter(0..colors_per_row)
                .try_serialized(MyPoints::descriptor_colors())
                .unwrap()],
        );
    }
    Arc::new(builder.build().unwrap())
}

/// A temporal chunk with both a points and a colors column on every row.
pub fn temporal_mixed_chunk(
    id: u128,
    entity: &str,
    times: &[i64],
    points_per_row: u32,
    colors_per_row: u32,
) -> Arc<Chunk> {
    let frame = Timeline::new_sequence("frame");
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id), entity);
    for (i, &time) in times.iter().enumerate() {
        builder = builder.with_serialized_batches(
            RowId::from_u128((id << 32) + i as u128 + 1),
            [(frame, time)],
            [
                MyPoint::from_iter(0..points_per_row)
                    .try_serialized(MyPoints::descriptor_points())
                    .unwrap(),
                MyColor::from_iter(0..colors_per_row)
                    .try_serialized(MyPoints::descriptor_colors())
                    .unwrap(),
            ],
        );
    }
    Arc::new(builder.build().unwrap())
}

pub fn points() -> ComponentIdentifier {
    MyPoints::descriptor_points().component
}

pub fn colors() -> ComponentIdentifier {
    MyPoints::descriptor_colors().component
}

pub fn columns_of(chunk: &Chunk) -> BTreeSet<ComponentIdentifier> {
    chunk.components().keys().copied().collect()
}

pub fn test_store_id() -> StoreId {
    StoreId::new(StoreKind::Recording, "test_app", "test_recording")
}

pub fn provider_of(chunks: impl IntoIterator<Item = Arc<Chunk>>) -> Arc<InMemoryChunkProvider> {
    Arc::new(InMemoryChunkProvider::new(&test_store_id(), chunks).unwrap())
}

/// Write `chunks` to an RRD file at `path`, in order.
pub fn write_rrd(path: &Path, store_id: &StoreId, chunks: &[Arc<Chunk>]) {
    let mut file = std::fs::File::create(path).unwrap();
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_log_encoding::CrateVersion::LOCAL,
        EncodingOptions::PROTOBUF_COMPRESSED,
        &mut file,
    )
    .unwrap();
    encoder
        .append(&LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *RowId::ZERO,
            info: StoreInfo::new(store_id.clone(), StoreSource::Unknown),
        }))
        .unwrap();
    for chunk in chunks {
        encoder
            .append(&LogMsg::ArrowMsg(
                store_id.clone(),
                chunk.to_arrow_msg().unwrap(),
            ))
            .unwrap();
    }
    encoder.finish().unwrap();
}

/// Open an RRD file written by [`write_rrd`] as a chunk provider.
pub fn file_provider(path: &Path, store_id: &StoreId) -> Arc<RrdChunkProvider<std::fs::File>> {
    let footer_file = std::fs::File::open(path).unwrap();
    let footer = block_on(re_log_encoding::read_rrd_footer(&footer_file))
        .unwrap()
        .unwrap();
    let raw = Arc::new(footer.manifests[store_id].clone());
    drop(footer_file);

    let file = std::fs::File::open(path).unwrap();
    Arc::new(RrdChunkProvider::from_reader(file, path.display().to_string(), raw).unwrap())
}

pub fn collect(stream: impl Stream<Item = Result<Arc<Chunk>, Error>>) -> Vec<Arc<Chunk>> {
    block_on(stream.try_collect()).unwrap()
}

/// Every `(entity, row id)` pair of `chunks`, for row-survival comparisons.
pub fn row_set(chunks: &[Arc<Chunk>]) -> BTreeSet<(String, RowId)> {
    chunks
        .iter()
        .flat_map(|chunk| {
            let entity = chunk.entity_path().to_string();
            chunk
                .row_ids()
                .map(move |row_id| (entity.clone(), row_id))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Every non-null `(entity, row id, column)` cell of `chunks`, for data-survival comparisons
/// across column splits, where a row id legitimately lands in several outputs.
///
/// Null cells are excluded: a merge of chunks with different column sets pads with nulls.
pub fn cell_set(chunks: &[Arc<Chunk>]) -> BTreeSet<(String, RowId, ComponentIdentifier)> {
    chunks
        .iter()
        .flat_map(|chunk| {
            let entity = chunk.entity_path().to_string();
            let row_ids: Vec<RowId> = chunk.row_ids().collect();
            chunk
                .components()
                .iter()
                .flat_map(|(&column, serialized)| {
                    let entity = entity.clone();
                    row_ids
                        .iter()
                        .enumerate()
                        .filter(move |&(i, _)| serialized.list_array.is_valid(i))
                        .map(move |(_, &row_id)| (entity.clone(), row_id, column))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn measured(chunk: &Arc<Chunk>) -> u64 {
    chunk.as_ref().total_size_bytes()
}

pub fn settings_max_row_unsorted(
    max_bytes: u64,
    max_rows: u64,
    max_rows_if_unsorted: u64,
) -> OptimizationSettings {
    OptimizationSettings {
        merge_split: Some(MergeSplitSettings {
            max_bytes: NonZeroU64::new(max_bytes).unwrap(),
            max_rows: NonZeroU64::new(max_rows),
            max_rows_if_unsorted: NonZeroU64::new(max_rows_if_unsorted),
        }),
        target_timeline: None,
        own_chunk: Vec::new(),
    }
}

pub fn settings(max_bytes: u64, max_rows: u64) -> OptimizationSettings {
    settings_max_row_unsorted(max_bytes, max_rows, 0)
}

pub fn merge_split_settings(max_bytes: u64) -> MergeSplitSettings {
    MergeSplitSettings {
        max_bytes: NonZeroU64::new(max_bytes).unwrap(),
        max_rows: None,
        max_rows_if_unsorted: None,
    }
}

pub fn colors_own_chunk_rule() -> OwnChunkRule {
    OwnChunkRule::new(ColumnSelector::Type(MyColor::name()))
}

pub fn with_rules(
    mut settings: OptimizationSettings,
    rules: Vec<OwnChunkRule>,
) -> OptimizationSettings {
    settings.own_chunk = rules;
    settings
}
