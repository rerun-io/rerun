use insta::Settings;
use re_chunk::{Chunk, ChunkId, RowId};
use re_log_types::example_components::{MyColor, MyIndex, MyPoints};
use re_log_types::{EntityPath, Timestamp, build_frame_nr, build_log_time};
use re_types_core::ComponentBatch as _;

fn create_chunk() -> anyhow::Result<Chunk> {
    let entity_path = EntityPath::from("this/that");

    let (indices1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));

    let chunk_id = ChunkId::from_u128(123_456_789_123_456_789_123_456_789);
    let row_id = RowId::from_u128(32_033_410_000_000_000_000_000_000_123);

    let chunk = Chunk::builder_with_id(chunk_id, entity_path.clone())
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
        .build()?;

    Ok(chunk)
}

#[test]
/// We don't use [`crate::RecordBatchFormatOpts::redact_non_deterministic`] here because
/// this should test printing `RowId` and `ChunkId`.
fn format_chunk() -> anyhow::Result<()> {
    let chunk = create_chunk()?;

    let mut settings = Settings::clone_current();
    // Replace the `version` number and `heap_size_bytes` by [`**REDACTED**`] and pad the new
    // string so that everything formats nicely.
    settings.add_filter(
        r"\* version: \d+\.\d+\.\d+(\s*)│",
        "* version: [**REDACTED**]<>│".replace("<>", &" ".repeat(150)),
    );
    settings.add_filter(
        r"\* heap_size_bytes: \d+(\s*)│",
        "* heap_size_bytes: [**REDACTED**]<>│".replace("<>", &" ".repeat(142)),
    );
    settings.bind(|| {
        insta::assert_snapshot!("format_chunk", format!("{:240}", chunk));
    });

    Ok(())
}

#[test]
fn format_chunk_redacted() -> anyhow::Result<()> {
    let chunk = create_chunk()?;

    insta::assert_snapshot!("format_chunk_redacted", format!("{:-240}", chunk));

    Ok(())
}
