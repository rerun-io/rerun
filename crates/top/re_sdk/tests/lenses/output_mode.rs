#![expect(clippy::unwrap_used)]

use arrow::array::{ListBuilder, StringBuilder};
use re_chunk::{Chunk, ChunkId, TimeColumn, TimelineName};
use re_sdk::lenses::{Lens, Lenses, OutputMode};
use re_sdk_types::ComponentDescriptor;

/// Helper to create a simple chunk with string data for testing
fn create_test_chunk(entity_path: &str, component_name: &str) -> Chunk {
    let mut builder = ListBuilder::new(StringBuilder::new());
    builder.values().append_value("test");
    builder.append(true);
    builder.values().append_value("data");
    builder.append(true);
    let list_array = builder.finish();

    let components = std::iter::once((ComponentDescriptor::partial(component_name), list_array));

    let time_column = TimeColumn::new_sequence("tick", 0..2);

    Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path.into(),
        std::iter::once((TimelineName::new("tick"), time_column)).collect(),
        components.collect(),
    )
    .unwrap()
}

#[test]
fn test_output_mode_forward_all() {
    // Create two chunks: one that matches the lens and one that doesn't
    let matching_chunk = create_test_chunk("matched/entity", "test_component");
    let unmatched_chunk = create_test_chunk("other/entity", "other_component");

    // Create a lens that only matches the first chunk
    let lens = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("matched/**"),
        "test_component",
    )
    .output_columns_at("matched/output", |out| {
        out.component(ComponentDescriptor::partial("transformed"), [])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::ForwardAll);
    lenses.add_lens(lens);

    // Apply to matching chunk
    let matching_results: Vec<_> = lenses
        .apply(&matching_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get both the transformed chunk AND the original chunk
    assert_eq!(matching_results.len(), 2);
    assert_eq!(matching_results[0].entity_path(), &"matched/output".into());
    assert_eq!(
        matching_results[1].entity_path(),
        matching_chunk.entity_path()
    );

    // Apply to unmatched chunk
    let unmatched_results: Vec<_> = lenses
        .apply(&unmatched_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get only the original chunk (no lens matched)
    assert_eq!(unmatched_results.len(), 1);
    assert_eq!(
        unmatched_results[0].entity_path(),
        unmatched_chunk.entity_path()
    );
}

#[test]
fn test_output_mode_forward_unmatched() {
    // Create two chunks: one that matches the lens and one that doesn't
    let matching_chunk = create_test_chunk("matched/entity", "test_component");
    let unmatched_chunk = create_test_chunk("other/entity", "other_component");

    // Create a lens that only matches the first chunk
    let lens = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("matched/**"),
        "test_component",
    )
    .output_columns_at("matched/output", |out| {
        out.component(ComponentDescriptor::partial("transformed"), [])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::ForwardUnmatched);
    lenses.add_lens(lens);

    // Apply to matching chunk
    let matching_results: Vec<_> = lenses
        .apply(&matching_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get only the transformed chunk (not the original)
    assert_eq!(matching_results.len(), 1);
    assert_eq!(matching_results[0].entity_path(), &"matched/output".into());

    // Apply to unmatched chunk
    let unmatched_results: Vec<_> = lenses
        .apply(&unmatched_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get the original chunk forwarded
    assert_eq!(unmatched_results.len(), 1);
    assert_eq!(
        unmatched_results[0].entity_path(),
        unmatched_chunk.entity_path()
    );
}

#[test]
fn test_output_mode_drop_unmatched() {
    // Create two chunks: one that matches the lens and one that doesn't
    let matching_chunk = create_test_chunk("matched/entity", "test_component");
    let unmatched_chunk = create_test_chunk("other/entity", "other_component");

    // Create a lens that only matches the first chunk
    let lens = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("matched/**"),
        "test_component",
    )
    .output_columns_at("matched/output", |out| {
        out.component(ComponentDescriptor::partial("transformed"), [])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(lens);

    // Apply to matching chunk
    let matching_results: Vec<_> = lenses
        .apply(&matching_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get only the transformed chunk
    assert_eq!(matching_results.len(), 1);
    assert_eq!(matching_results[0].entity_path(), &"matched/output".into());

    // Apply to unmatched chunk
    let unmatched_results: Vec<_> = lenses
        .apply(&unmatched_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    // Should get nothing (unmatched data is dropped)
    assert_eq!(unmatched_results.len(), 0);
}
