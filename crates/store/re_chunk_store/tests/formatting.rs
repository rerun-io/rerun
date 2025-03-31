use std::sync::Arc;

use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::ChunkStore;
use re_log_types::{
    build_frame_nr, build_log_time,
    example_components::{MyColor, MyIndex},
    EntityPath, Timestamp,
};
use re_types_core::ComponentBatch as _;

/// Ensure that `ChunkStore::to_string()` is nice and readable.
#[test]
fn format_chunk_store() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::from_string(
            re_log_types::StoreKind::Recording,
            "test_id".to_owned(),
        ),
        Default::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let (indices1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));

    let chunk_id = ChunkId::from_u128(123_456_789_123_456_789_123_456_789);
    let row_id = RowId::from_u128(32_033_410_000_000_000_000_000_000_123);

    store.insert_chunk(&Arc::new(
        Chunk::builder_with_id(chunk_id, entity_path.clone())
            .with_serialized_batches(
                row_id,
                [
                    build_frame_nr(1),
                    build_log_time(Timestamp::from_nanos_since_epoch(1_736_534_622_123_456_789)),
                ],
                [indices1.try_serialized()?, colors1.try_serialized()?],
            )
            .build()?,
    ))?;

    insta::assert_snapshot!("format_chunk_store", format!("{:200}", store));

    Ok(())
}
