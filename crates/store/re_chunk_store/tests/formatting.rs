use std::sync::Arc;

use insta::Settings;
use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::ChunkStore;
use re_log_types::example_components::{MyColor, MyIndex, MyPoints};
use re_log_types::{ApplicationId, EntityPath, Timestamp, build_frame_nr, build_log_time};
use re_types_core::ComponentBatch as _;

/// Ensure that `ChunkStore::to_string()` is nice and readable.
#[test]
fn format_chunk_store() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::new(
            re_log_types::StoreKind::Recording,
            ApplicationId::from("test_app"),
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
                [
                    indices1.try_serialized(MyIndex::partial_descriptor())?,
                    colors1.try_serialized(MyPoints::descriptor_colors())?,
                ],
            )
            .build()?,
    ))?;

    let mut settings = Settings::clone_current();
    // Replace the version number by [`**REDACTED**`] and pad the new string so that everything formats nicely.
    settings.add_filter(
        r"\* version: \d+\.\d+\.\d+(\s*)│",
        "* version: [**REDACTED**]<>│".replace("<>", &" ".repeat(149)),
    );
    settings.add_filter(
        r"\* heap_size_bytes: \d+(\s*)│",
        "* heap_size_bytes: [**REDACTED**]<>│".replace("<>", &" ".repeat(142)),
    );
    settings.bind(|| {
        insta::assert_snapshot!("format_chunk_store", format!("{:240}", store));
    });

    Ok(())
}
