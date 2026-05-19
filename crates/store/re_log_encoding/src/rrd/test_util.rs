use std::path::Path;
use std::sync::Arc;

use re_chunk::{Chunk, RowId, TimePoint, Timeline};
use re_log_types::{
    EntityPath, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource,
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
            let points = MyPoint::from_iter(i as u32..i as u32 + 1);
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
