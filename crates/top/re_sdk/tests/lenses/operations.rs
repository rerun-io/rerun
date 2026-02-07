#![expect(clippy::cast_possible_wrap)]
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{AsArray as _, Int32Builder, ListArray, ListBuilder, StringBuilder};
use arrow::datatypes::{DataType, Field};
use re_chunk::{ArrowArray as _, Chunk, ChunkId, TimeColumn, TimelineName};
use re_sdk::lenses::{Lens, Lenses, Op, OutputMode};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::Scalars;

/// Helper to convert serializable data to a `ListArray` using Arrow's JSON decoder
fn to_list_array<T: serde::Serialize>(data: &[T], inner_field: Arc<Field>) -> ListArray {
    use arrow::json::ReaderBuilder;

    let list_field = Arc::new(Field::new_list_field(DataType::List(inner_field), true));
    let schema = Arc::new(arrow::datatypes::Schema::new_with_metadata(
        vec![list_field],
        Default::default(),
    ));

    // Wrap each row in an object with "item" field
    let rows: Vec<_> = data
        .iter()
        .map(|row| serde_json::json!({ "item": row }))
        .collect();

    let mut decoder = ReaderBuilder::new(schema).build_decoder().unwrap();
    decoder.serialize(&rows).unwrap();

    let batch = decoder.flush().unwrap().unwrap();
    batch.column(0).as_list::<i32>().clone()
}

/// Creates a chunk that contains all sorts of validity, nullability, and empty lists.
///
/// # Layout
/// ```text
/// ┌──────────────┬───────────┐
/// │ [{a:0,b:0}]  │ ["zero"]  │
/// ├──────────────┼───────────┤
/// │[{a:1,b:null}]│["one","1"]│
/// ├──────────────┼───────────┤
/// │      []      │    []     │
/// ├──────────────┼───────────┤
/// │     null     │ ["three"] │
/// ├──────────────┼───────────┤
/// │ [{a:4,b:4}]  │   null    │
/// ├──────────────┼───────────┤
/// │    [null]    │ ["five"]  │
/// ├──────────────┼───────────┤
/// │ [{a:6,b:6}]  │  [null]   │
/// └──────────────┴───────────┘
/// ```
fn nullability_chunk() -> Chunk {
    #[derive(serde::Serialize)]
    struct MyStruct {
        a: Option<f32>,
        b: Option<f64>,
    }

    let struct_field = Arc::new(Field::new(
        "item",
        DataType::Struct(
            vec![
                Arc::new(Field::new("a", DataType::Float32, true)),
                Arc::new(Field::new("b", DataType::Float64, true)),
            ]
            .into(),
        ),
        true,
    ));

    let string_field = Arc::new(Field::new("item", DataType::Utf8, true));

    let struct_data = vec![
        Some(vec![Some(MyStruct {
            a: Some(0.0),
            b: Some(0.0),
        })]),
        Some(vec![Some(MyStruct {
            a: Some(1.0),
            b: None,
        })]),
        Some(vec![]),
        None,
        Some(vec![Some(MyStruct {
            a: Some(4.0),
            b: Some(4.0),
        })]),
        Some(vec![None]),
        Some(vec![Some(MyStruct {
            a: Some(6.0),
            b: Some(6.0),
        })]),
    ];

    let string_data = vec![
        Some(vec![Some("zero")]),
        Some(vec![Some("one"), Some("1")]),
        Some(vec![]),
        Some(vec![Some("three")]),
        None,
        Some(vec![Some("five")]),
        Some(vec![None]),
    ];

    let struct_column = to_list_array(&struct_data, struct_field);
    let string_column = to_list_array(&string_data, string_field);

    let components = [
        (ComponentDescriptor::partial("structs"), struct_column),
        (ComponentDescriptor::partial("strings"), string_column),
    ]
    .into_iter();

    let time_column = TimeColumn::new_sequence("tick", [0, 1, 2, 3, 4, 5, 6]);

    Chunk::from_auto_row_ids(
        ChunkId::new(),
        "nullability".into(),
        std::iter::once((TimelineName::new("tick"), time_column)).collect(),
        components.collect(),
    )
    .unwrap()
}

#[test]
fn test_destructure_cast() {
    let original_chunk = nullability_chunk();
    println!("{original_chunk}");

    let destructure = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("nullability"),
        "structs",
    )
    .output_columns_at("nullability/a", |out| {
        out.component(
            Scalars::descriptor_scalars(),
            [Op::selector(".a"), Op::cast(DataType::Float64)],
        )
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(destructure);

    let res: Vec<re_chunk::Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();

    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    insta::assert_snapshot!("destructure_cast", format!("{chunk:-240}"));
}

#[test]
fn test_destructure() {
    let original_chunk = nullability_chunk();
    println!("{original_chunk}");

    let destructure = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("nullability"),
        "structs",
    )
    .output_columns_at("nullability/b", |out| {
        out.component(Scalars::descriptor_scalars(), [Op::selector(".b")])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(destructure);

    let res: Vec<re_chunk::Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    insta::assert_snapshot!("destructure_only", format!("{chunk:-240}"));
}

#[test]
fn test_inner_count() {
    use re_sdk::lenses::OpError;

    let original_chunk = nullability_chunk();
    println!("{original_chunk}");

    let count_fn = |list_array: &ListArray| -> Result<ListArray, OpError> {
        let mut builder = ListBuilder::new(Int32Builder::new());

        for maybe_array in list_array.iter() {
            match maybe_array {
                None => builder.append_null(),
                Some(component_batch_array) => {
                    builder
                        .values()
                        .append_value(component_batch_array.len() as i32);
                    builder.append(true);
                }
            }
        }

        Ok(builder.finish())
    };

    let count = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("nullability"),
        "strings",
    )
    .output_columns(|out| {
        out.component(ComponentDescriptor::partial("counts"), [Op::func(count_fn)])
            .component(ComponentDescriptor::partial("original"), [])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(count);

    let res: Vec<re_chunk::Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    insta::assert_snapshot!("inner_count", format!("{chunk:-240}"));
}

#[test]
fn test_static_chunk_creation() {
    let original_chunk = nullability_chunk();

    let mut metadata_builder_a = ListBuilder::new(StringBuilder::new());
    metadata_builder_a
        .values()
        .append_value("static_metadata_a");
    metadata_builder_a.append(true);

    let mut metadata_builder_b = ListBuilder::new(StringBuilder::new());
    metadata_builder_b
        .values()
        .append_value("static_metadata_b");
    metadata_builder_b.append(true);

    let static_lens = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("nullability"),
        "strings",
    )
    .output_static_columns_at("nullability/static", |out| {
        out.component(
            ComponentDescriptor::partial("static_metadata_a"),
            [Op::constant(metadata_builder_a.finish())],
        )
        .component(
            ComponentDescriptor::partial("static_metadata_b"),
            [Op::constant(metadata_builder_b.finish())],
        )
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(static_lens);

    let res: Vec<re_chunk::Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    insta::assert_snapshot!("single_static", format!("{chunk:-240}"));
}

#[test]
fn test_time_column_extraction() {
    use re_log_types::TimeType;

    // Create a chunk with timestamp data that can be extracted as a time column
    let mut timestamp_builder = ListBuilder::new(arrow::array::Int64Builder::new());
    let mut value_builder = ListBuilder::new(Int32Builder::new());

    // Add rows with timestamps and corresponding values
    for i in 0..5 {
        timestamp_builder.values().append_value(100 + i * 10);
        timestamp_builder.append(true);

        value_builder.values().append_value(i as i32);
        value_builder.append(true);
    }

    let timestamp_column = timestamp_builder.finish();
    let value_column = value_builder.finish();

    let components = [
        (
            ComponentDescriptor::partial("my_timestamp"),
            timestamp_column,
        ),
        (ComponentDescriptor::partial("value"), value_column),
    ]
    .into_iter();

    // Create chunk without the custom timeline initially
    let time_column = TimeColumn::new_sequence("tick", [0, 1, 2, 3, 4]);

    let original_chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        "timestamped".into(),
        std::iter::once((TimelineName::new("tick"), time_column)).collect(),
        components.collect(),
    )
    .unwrap();

    println!("{original_chunk}");

    // Create a lens that extracts the timestamp as a time column and keeps the original timestamp as a component
    let time_lens = Lens::for_input_column(
        re_log_types::EntityPathFilter::parse_forgiving("timestamped"),
        "my_timestamp",
    )
    .output_columns(|out| {
        out.time("my_timeline", TimeType::Sequence, [])
            .component(ComponentDescriptor::partial("extracted_time"), [])
    })
    .unwrap()
    .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(time_lens);

    let res: Vec<Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    println!("{chunk}");

    // Verify the chunk has both the original timeline and the new custom timeline
    assert!(chunk.timelines().contains_key(&TimelineName::new("tick")));
    assert!(
        chunk
            .timelines()
            .contains_key(&TimelineName::new("my_timeline"))
    );

    // Verify the custom timeline has the correct values
    let my_timeline = chunk
        .timelines()
        .get(&TimelineName::new("my_timeline"))
        .unwrap();
    assert_eq!(my_timeline.times_raw().len(), 5);
    assert_eq!(my_timeline.times_raw()[0], 100);
    assert_eq!(my_timeline.times_raw()[1], 110);
    assert_eq!(my_timeline.times_raw()[2], 120);
    assert_eq!(my_timeline.times_raw()[3], 130);
    assert_eq!(my_timeline.times_raw()[4], 140);
}

// Helper function to create test data: list of structs with {timestamp: i64, value: String}
fn create_test_struct_list() -> arrow::array::ListArray {
    #[derive(serde::Serialize)]
    struct TimestampedValue {
        timestamp: i64,
        value: Option<String>,
    }

    let struct_field = Arc::new(Field::new(
        "item",
        DataType::Struct(
            vec![
                Arc::new(Field::new("timestamp", DataType::Int64, true)),
                Arc::new(Field::new("value", DataType::Utf8, true)),
            ]
            .into(),
        ),
        true,
    ));

    let data = vec![
        vec![
            TimestampedValue {
                timestamp: 1,
                value: Some("one".to_owned()),
            },
            TimestampedValue {
                timestamp: 2,
                value: Some("two".to_owned()),
            },
            TimestampedValue {
                timestamp: 3,
                value: Some("three".to_owned()),
            },
        ],
        vec![TimestampedValue {
            timestamp: 4,
            value: Some("four".to_owned()),
        }],
        vec![TimestampedValue {
            timestamp: 5,
            value: None,
        }],
    ];

    to_list_array(&data, struct_field)
}

#[test]
fn test_scatter_columns() {
    use re_arrow_combinators::{Selector, Transform as _};
    use re_log_types::TimeType;
    use re_sdk::lenses::OpError;
    use std::str::FromStr as _;

    // Create a chunk with list of structs that should be exploded/scattered
    // Each element is a struct with {timestamp: i64, value: String}
    let struct_list = create_test_struct_list();

    let components = std::iter::once((ComponentDescriptor::partial("nested_data"), struct_list));

    let time_column = TimeColumn::new_sequence("tick", [1, 2, 3]);

    let original_chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        "scatter_test".into(),
        std::iter::once((time_column.timeline().name().to_owned(), time_column)).collect(),
        components.collect(),
    )
    .unwrap();

    println!("Original chunk:");
    println!("{original_chunk}");

    // Helper to extract value field from structs: List<Struct> -> List<String>
    let extract_value = |list_array: &ListArray| -> Result<ListArray, OpError> {
        Ok(Selector::from_str(".value")?.transform(list_array)?)
    };

    // Helper to extract timestamp field from structs: List<Struct> -> List<Int64>
    let extract_timestamp = |list_array: &ListArray| -> Result<ListArray, OpError> {
        Ok(Selector::from_str(".timestamp")?.transform(list_array)?)
    };

    // Create a scatter lens that explodes the nested lists
    let scatter_lens = Lens::for_input_column(re_log_types::EntityPathFilter::all(), "nested_data")
        .output_scatter_columns_at("scatter_test/exploded", |out| {
            out.component(
                ComponentDescriptor::partial("exploded_strings"),
                [Op::func(extract_value)],
            )
            .time(
                "my_timestamp",
                TimeType::Sequence,
                [Op::func(extract_timestamp)],
            )
        })
        .unwrap()
        .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(scatter_lens);

    let res: Vec<Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    println!("\nExploded chunk:");
    println!("{chunk}");

    // Verify the structure
    // Input had 3 rows with list of structs:
    // Row 0: [{1, "one"}, {2, "two"}, {3, "three"}] → 3 output rows
    // Row 1: [{4, "four"}] → 1 output row
    // Row 2: [{5, null}] → 1 output row
    // Total: 5 output rows
    assert_eq!(chunk.num_rows(), 5);

    // Verify tick timeline is replicated correctly
    // Original tick: [1, 2, 3]
    // Scattered tick: [1, 1, 1, 2, 3] (row 0 scatters into 3 rows)
    let tick_timeline = chunk.timelines().get(&TimelineName::new("tick")).unwrap();
    assert_eq!(tick_timeline.times_raw().len(), 5);
    assert_eq!(tick_timeline.times_raw()[0], 1);
    assert_eq!(tick_timeline.times_raw()[1], 1);
    assert_eq!(tick_timeline.times_raw()[2], 1);
    assert_eq!(tick_timeline.times_raw()[3], 2);
    assert_eq!(tick_timeline.times_raw()[4], 3);

    // Verify my_timestamp timeline is extracted from the timestamp field
    // The timestamps are: 1, 2, 3 (from row 0), 4 (row 1), 5 (row 2)
    // After scattering: [1, 2, 3, 4, 5]
    let event_timeline = chunk
        .timelines()
        .get(&TimelineName::new("my_timestamp"))
        .unwrap();
    assert_eq!(event_timeline.times_raw().len(), 5);
    assert_eq!(event_timeline.times_raw()[0], 1);
    assert_eq!(event_timeline.times_raw()[1], 2);
    assert_eq!(event_timeline.times_raw()[2], 3);
    assert_eq!(event_timeline.times_raw()[3], 4);
    assert_eq!(event_timeline.times_raw()[4], 5);

    insta::assert_snapshot!("scatter_columns", format!("{chunk:-240}"));
}

#[test]
fn test_scatter_columns_static() {
    use re_arrow_combinators::{Selector, Transform as _};
    use re_log_types::TimeType;
    use re_sdk::lenses::OpError;
    use std::str::FromStr as _;

    // Test scatter with no existing timelines - only exploded timeline outputs
    let struct_list = create_test_struct_list();

    let components = std::iter::once((ComponentDescriptor::partial("nested_data"), struct_list));

    // Create chunk WITHOUT any timelines
    let original_chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        "scatter_test".into(),
        std::iter::empty().collect(), // No timelines!
        components.collect(),
    )
    .unwrap();

    println!("Original chunk (no timelines):");
    println!("{original_chunk}");

    // Helper to extract value field from structs: List<Struct> -> List<String>
    let extract_value = |list_array: &ListArray| -> Result<ListArray, OpError> {
        Ok(Selector::from_str(".value")?.transform(list_array)?)
    };

    // Helper to extract timestamp field from structs: List<Struct> -> List<Int64>
    let extract_timestamp = |list_array: &ListArray| -> Result<ListArray, OpError> {
        Ok(Selector::from_str(".timestamp")?.transform(list_array)?)
    };

    // Create a scatter lens that explodes the nested lists
    let scatter_lens = Lens::for_input_column(re_log_types::EntityPathFilter::all(), "nested_data")
        .output_scatter_columns_at("scatter_test/exploded", |out| {
            out.component(
                ComponentDescriptor::partial("exploded_strings"),
                [Op::func(extract_value)],
            )
            .time(
                "my_timestamp",
                TimeType::Sequence,
                [Op::func(extract_timestamp)],
            )
        })
        .unwrap()
        .build();

    let mut lenses = Lenses::new(OutputMode::DropUnmatched);
    lenses.add_lens(scatter_lens);

    let res: Vec<Chunk> = lenses
        .apply(&original_chunk)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(res.len(), 1);

    let chunk = &res[0];
    println!("\nExploded chunk:");
    println!("{chunk}");

    // Verify the structure
    // Input had 3 rows with list of structs:
    // Row 0: [{1, "one"}, {2, "two"}, {3, "three"}] → 3 output rows
    // Row 1: [{4, "four"}] → 1 output row
    // Row 2: [{5, null}] → 1 output row
    // Total: 5 output rows
    assert_eq!(chunk.num_rows(), 5);

    // Verify there are NO scattered timelines from input (since input had none)
    // Only the exploded my_timestamp timeline should exist
    assert_eq!(chunk.timelines().len(), 1);

    // Verify my_timestamp timeline is extracted from the timestamp field
    // The timestamps are: 1, 2, 3 (from row 0), 4 (row 1), 5 (row 2)
    // After scattering: [1, 2, 3, 4, 5]
    let event_timeline = chunk
        .timelines()
        .get(&TimelineName::new("my_timestamp"))
        .unwrap();
    assert_eq!(event_timeline.times_raw().len(), 5);
    assert_eq!(event_timeline.times_raw()[0], 1);
    assert_eq!(event_timeline.times_raw()[1], 2);
    assert_eq!(event_timeline.times_raw()[2], 3);
    assert_eq!(event_timeline.times_raw()[3], 4);
    assert_eq!(event_timeline.times_raw()[4], 5);

    // Verify exploded_strings component exists
    let strings_component = chunk
        .components()
        .get(ComponentDescriptor::partial("exploded_strings").component)
        .unwrap();
    assert_eq!(strings_component.list_array.len(), 5);

    insta::assert_snapshot!("scatter_columns_static", format!("{chunk:-240}"));
}
