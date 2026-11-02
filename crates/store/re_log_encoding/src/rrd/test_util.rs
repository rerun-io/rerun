use std::path::Path;
use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_msg::{LogMsg, SetStoreInfo, StoreInfo, StoreSource};
use re_log_types::{
    EntityPath, StoreId, StoreKind,
    example_components::{MyPoint, MyPoints},
};
use tempfile::NamedTempFile;

/// Create simple test chunks with temporal data.
pub fn make_test_chunks(num_chunks: usize) -> Vec<Arc<Chunk>> {
    let entity_path = EntityPath::from("/test/entity");
    let timeline = Timeline::new_sequence("frame");

    (0..num_chunks)
        .map(|i| {
            let row_id = RowId::new();
            let points = MyPoint::from_iter((i as u32)..=(i as u32));
            let chunk = Chunk::builder(entity_path.clone())
                .with_sparse_component_batches(
                    row_id,
                    #[expect(clippy::cast_possible_wrap)]
                    TimePoint::default().with(timeline, i as i64),
                    [(MyPoints::descriptor_points(), Some(&points as _))],
                )
                .build()
                .unwrap();
            Arc::new(chunk)
        })
        .collect()
}

/// Points whose coordinates are deterministic pseudo-random values, which LZ4 cannot shrink.
///
/// Stands in for incompressible payloads such as encoded video.
pub fn make_incompressible_chunk(num_points: usize) -> Arc<Chunk> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 8) as f32
    };
    let points: Vec<MyPoint> = (0..num_points)
        .map(|_| MyPoint::new(next(), next()))
        .collect();
    make_points_chunk(&points)
}

/// Points that are all identical, which LZ4 shrinks to almost nothing.
pub fn make_compressible_chunk(num_points: usize) -> Arc<Chunk> {
    let points = vec![MyPoint::new(1.0, 2.0); num_points];
    make_points_chunk(&points)
}

fn make_points_chunk(points: &dyn re_types_core::ComponentBatch) -> Arc<Chunk> {
    let chunk = Chunk::builder("/test/entity")
        .with_sparse_component_batches(
            RowId::new(),
            TimePoint::default().with(Timeline::new_sequence("frame"), 0),
            [(MyPoints::descriptor_points(), Some(points))],
        )
        .build()
        .unwrap();
    Arc::new(chunk)
}

/// Encode test chunks into a temporary RRD file (with footer).
///
/// Returns the temp file (keeps it alive) and the `StoreId`.
/// Use `.path()` on the returned file to get the path.
pub fn encode_test_rrd(chunks: &[Arc<Chunk>]) -> (NamedTempFile, StoreId) {
    let file = NamedTempFile::new().unwrap();
    let store_id = encode_test_rrd_to_file(file.path(), chunks, true);
    (file, store_id)
}

/// Encode chunks into an RRD file at the given path. Returns the `StoreId` used.
pub fn encode_test_rrd_to_file(path: &Path, chunks: &[Arc<Chunk>], with_footer: bool) -> StoreId {
    let store_id = StoreId::random(StoreKind::Recording, "test");
    encode_test_rrd_to_file_with_options(
        path,
        chunks,
        &store_id,
        with_footer,
        crate::EncodingOptions::PROTOBUF_COMPRESSED,
    );
    store_id
}

/// Encode chunks with specific options.
pub fn encode_test_rrd_to_file_with_options(
    path: &Path,
    chunks: &[Arc<Chunk>],
    store_id: &StoreId,
    with_footer: bool,
    options: crate::EncodingOptions,
) {
    let store_info = StoreInfo::new(store_id.clone(), StoreSource::Unknown);
    let set_store_info = LogMsg::SetStoreInfo(SetStoreInfo {
        row_id: *RowId::ZERO,
        info: store_info,
    });

    let mut file = std::fs::File::create(path).unwrap();
    let mut encoder =
        crate::Encoder::new_eager(re_build_info::CrateVersion::LOCAL, options, &mut file).unwrap();
    if !with_footer {
        encoder.do_not_emit_footer();
    }
    encoder.append(&set_store_info).unwrap();
    for chunk in chunks {
        let arrow_msg = chunk.to_arrow_msg().unwrap();
        let msg = LogMsg::ArrowMsg(store_id.clone(), arrow_msg);
        encoder.append(&msg).unwrap();
    }
    encoder.finish().unwrap();
}
