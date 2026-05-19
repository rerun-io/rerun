use re_chunk::{Chunk, RowId, Timeline};
use re_log_types::example_components::{MyLabel, MyPoints};

#[test]
fn with_mapped_component_creates_new_ids() -> anyhow::Result<()> {
    let row_id = RowId::new();
    let timepoint = [(Timeline::new_sequence("frame"), 1)];
    let labels = &[MyLabel("hello".into())];

    let chunk = Chunk::builder("my/entity")
        .with_component_batches(
            row_id,
            timepoint,
            [(MyPoints::descriptor_labels(), labels as _)],
        )
        .build()?;

    let mapped =
        chunk.with_mapped_component(MyPoints::descriptor_labels().component, None, |arr| {
            Ok::<_, std::convert::Infallible>(arr)
        })?;

    assert_ne!(mapped.id(), chunk.id());

    let old_row_ids: Vec<_> = chunk.row_ids().collect();
    let new_row_ids: Vec<_> = mapped.row_ids().collect();
    assert_eq!(old_row_ids.len(), new_row_ids.len());
    assert_ne!(old_row_ids, new_row_ids);

    Ok(())
}
