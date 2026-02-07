//! This testdata was mostly written by claude just to have a large variety of types

use std::sync::Arc;

use arrow::array::{
    Array, BinaryArray, BooleanArray, DictionaryArray, FixedSizeListArray, Float64Array,
    GenericListArray, Int32Array, Int32Builder, Int64Array, LargeBinaryArray, MapArray, MapBuilder,
    StringArray, StringBuilder, StructArray, UnionArray,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields, UnionFields};
use re_arrow_util::concat_arrays;

/// Creates a basic struct array with coordinates (x, y, z)
pub fn create_coordinates_struct(step: i32) -> Arc<dyn Array> {
    let step_f64 = step as f64;

    let x_values = Float64Array::from_iter([step_f64, step_f64 + 1.0, step_f64 + 2.0]);
    let y_values = Float64Array::from_iter([
        step_f64.sin(),
        (step_f64 + 1.0).sin(),
        (step_f64 + 2.0).sin(),
    ]);
    let z_values = Float64Array::from_iter([
        step_f64.cos(),
        (step_f64 + 1.0).cos(),
        (step_f64 + 2.0).cos(),
    ]);

    Arc::new(StructArray::from(vec![
        (
            Arc::new(Field::new("x", DataType::Float64, false)),
            Arc::new(x_values) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("y", DataType::Float64, false)),
            Arc::new(y_values) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("z", DataType::Float64, false)),
            Arc::new(z_values) as Arc<dyn Array>,
        ),
    ]))
}

/// Creates a metadata struct with string fields
pub fn create_metadata_struct(step: i32) -> Arc<dyn Array> {
    let timestamp = StringArray::from_iter([Some(format!("{step}"))]);
    let device_id = StringArray::from_iter([Some(format!("device_{}", step % 5))]);
    let sensor_type = StringArray::from_iter([Some("accelerometer".to_owned())]);

    Arc::new(StructArray::from(vec![
        (
            Arc::new(Field::new("timestamp", DataType::Utf8, false)),
            Arc::new(timestamp) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("device_id", DataType::Utf8, false)),
            Arc::new(device_id) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("sensor_type", DataType::Utf8, false)),
            Arc::new(sensor_type) as Arc<dyn Array>,
        ),
    ]))
}

/// Creates a list array with mixed primitive types (integers, strings, booleans)
pub fn create_mixed_lists(step: i32) -> Arc<dyn Array> {
    let integers_list =
        GenericListArray::<i32>::from_iter_primitive::<arrow::datatypes::Int32Type, _, _>([Some(
            vec![Some(step), Some(step * 2), Some(step * 3)],
        )]);

    let strings_data = StringArray::from_iter([
        Some(format!("step_{step}")),
        Some(format!("value_{}", (step as f64).sin())),
        Some(format!("id_{}", step % 10)),
    ]);
    let strings_list = GenericListArray::<i32>::new(
        Arc::new(Field::new("item", DataType::Utf8, true)),
        OffsetBuffer::from_lengths([3]),
        Arc::new(strings_data),
        None,
    );

    let booleans_data = BooleanArray::from_iter([
        Some(step % 2 == 0),
        Some(step % 3 == 0),
        Some(step % 5 == 0),
    ]);
    let booleans_list = GenericListArray::<i32>::new(
        Arc::new(Field::new("item", DataType::Boolean, true)),
        OffsetBuffer::from_lengths([3]),
        Arc::new(booleans_data),
        None,
    );

    Arc::new(StructArray::from(vec![
        (
            Arc::new(Field::new(
                "integers",
                DataType::List(Arc::new(Field::new("item", DataType::Int32, true))),
                false,
            )),
            Arc::new(integers_list) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new(
                "strings",
                DataType::List(Arc::new(Field::new("item", DataType::Utf8, true))),
                false,
            )),
            Arc::new(strings_list) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new(
                "booleans",
                DataType::List(Arc::new(Field::new("item", DataType::Boolean, true))),
                false,
            )),
            Arc::new(booleans_list) as Arc<dyn Array>,
        ),
    ]))
}

/// Creates a union array with different variant types
pub fn create_union_array(step: i32) -> Arc<dyn Array> {
    let step_f64 = step as f64;

    let type_ids = arrow::buffer::ScalarBuffer::from(vec![(step % 5) as i8]);
    let value_offsets = None; // Dense union doesn't use offsets

    let int_values = Int32Array::from_iter([Some(step * 42)]);
    let string_values = StringArray::from_iter([Some(format!("union_text_step_{step}"))]);
    let float_values = Float64Array::from_iter([Some(step_f64 * std::f64::consts::PI)]);
    let bool_values = BooleanArray::from_iter([Some(step % 3 == 0)]);

    let list_values =
        GenericListArray::<i32>::from_iter_primitive::<arrow::datatypes::Float64Type, _, _>([
            Some(vec![
                Some(step_f64),
                Some(step_f64 * 2.0),
                Some(step_f64 * 3.0),
                Some(step_f64.sin()),
                Some(step_f64.cos()),
            ]),
        ]);

    let union_fields = UnionFields::new(
        vec![0, 1, 2, 3, 4],
        vec![
            Field::new("int_variant", DataType::Int32, false),
            Field::new("string_variant", DataType::Utf8, false),
            Field::new("float_variant", DataType::Float64, false),
            Field::new("bool_variant", DataType::Boolean, false),
            Field::new(
                "list_variant",
                DataType::List(Arc::new(Field::new("item", DataType::Float64, true))),
                false,
            ),
        ],
    );

    let children = vec![
        Arc::new(int_values) as Arc<dyn Array>,
        Arc::new(string_values) as Arc<dyn Array>,
        Arc::new(float_values) as Arc<dyn Array>,
        Arc::new(bool_values) as Arc<dyn Array>,
        Arc::new(list_values) as Arc<dyn Array>,
    ];

    Arc::new(
        UnionArray::try_new(union_fields, type_ids, value_offsets, children)
            .expect("Failed to create union array"),
    )
}

/// Creates a map array with string keys and integer values
pub fn create_simple_map(step: i32) -> Arc<dyn Array> {
    let mut builder = MapBuilder::new(None, StringBuilder::new(), Int32Builder::with_capacity(4));

    builder.keys().append_value("blogs");
    builder.values().append_value(step * 2);
    builder.keys().append_value("foo");
    builder.values().append_value(step * 4);
    builder.keys().append_value("bar");
    builder.values().append_value(step * 6);
    builder
        .append(true)
        .expect("Failed to append to map builder");

    Arc::new(builder.finish())
}

/// Creates a map array with boolean keys and list values
pub fn create_complex_map(step: i32) -> Arc<dyn Array> {
    let step_f64 = step as f64;

    let keys = BooleanArray::from_iter([Some(true), Some(false)]);

    let true_list =
        GenericListArray::<i32>::from_iter_primitive::<arrow::datatypes::Float64Type, _, _>([
            Some(vec![
                Some(step_f64 * 10.0),
                Some(step_f64 * 20.0),
                Some(step_f64 * 30.0),
                Some(step_f64.sin() * 100.0),
                Some(step_f64.cos() * 100.0),
            ]),
        ]);

    let false_list =
        GenericListArray::<i32>::from_iter_primitive::<arrow::datatypes::Float64Type, _, _>([
            Some(vec![
                Some(-step_f64),
                Some(-step_f64 * 2.0),
                Some(-step_f64 * 3.0),
                Some(step_f64.ln().abs()),
            ]),
        ]);

    let all_list_values = [true_list.value(0).clone(), false_list.value(0).clone()];
    let combined_list = GenericListArray::<i32>::new(
        Arc::new(Field::new("item", DataType::Float64, true)),
        OffsetBuffer::from_lengths([5, 4]),
        Arc::new(
            concat_arrays(
                &all_list_values
                    .iter()
                    .map(|x| x.as_ref())
                    .collect::<Vec<_>>(),
            )
            .expect("Failed to concat arrays"),
        ),
        None,
    );

    Arc::new(MapArray::new(
        Arc::new(Field::new(
            "entries",
            DataType::Struct(Fields::from(vec![
                Field::new("key", DataType::Boolean, false),
                Field::new(
                    "value",
                    DataType::List(Arc::new(Field::new("item", DataType::Float64, true))),
                    false,
                ),
            ])),
            false,
        )),
        OffsetBuffer::from_lengths([2]),
        StructArray::from(vec![
            (
                Arc::new(Field::new("key", DataType::Boolean, false)),
                Arc::new(keys) as Arc<dyn Array>,
            ),
            (
                Arc::new(Field::new(
                    "value",
                    DataType::List(Arc::new(Field::new("item", DataType::Float64, true))),
                    false,
                )),
                Arc::new(combined_list) as Arc<dyn Array>,
            ),
        ]),
        None,
        false,
    ))
}

/// Creates a dictionary array with string dictionary and int indices
pub fn create_dictionary_array(step: i32) -> Arc<dyn Array> {
    let dictionary = StringArray::from_iter([
        Some("device_type_accelerometer".to_owned()),
        Some("device_type_gyroscope".to_owned()),
        Some("device_type_magnetometer".to_owned()),
        Some("device_type_barometer".to_owned()),
        Some("device_type_temperature".to_owned()),
        Some("device_type_humidity".to_owned()),
        Some("device_type_light_sensor".to_owned()),
        Some("device_type_proximity".to_owned()),
    ]);

    let indices = Int32Array::from_iter([Some(step % 8), Some(step % 3), Some(step % 6)]);

    Arc::new(
        DictionaryArray::try_new(indices, Arc::new(dictionary))
            .expect("Failed to create dictionary array"),
    )
}

/// Creates a binary array with various byte patterns
pub fn create_binary_array(step: i32) -> Arc<dyn Array> {
    Arc::new(BinaryArray::from_iter([
        Some(b"Hello, binary world!".as_slice()),
        Some(&[0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD, 0xFC]),
        Some(format!("step_{step}_binary_data").as_bytes()),
    ]))
}

/// Creates a large binary array (i64 offsets) with larger data
pub fn create_large_binary_array(step: i32) -> Arc<dyn Array> {
    Arc::new(LargeBinaryArray::from_iter([
        Some(b"This is large binary data with i64 offsets".as_slice()),
        Some(&vec![step as u8; 1000]), // 1000 bytes with the step value
        Some(format!("large_binary_step_{step}").repeat(10).as_bytes()),
    ]))
}

/// Creates a fixed-size list array
pub fn create_fixed_size_list(step: i32) -> Arc<dyn Array> {
    let step_f64 = step as f64;
    let values = Float64Array::from_iter([
        step_f64,
        step_f64 * 2.0,
        step_f64 * 3.0,
        step_f64.sin(),
        step_f64.cos(),
        step_f64.tan(),
    ]);

    Arc::new(FixedSizeListArray::new(
        Arc::new(Field::new("item", DataType::Float64, false)),
        3, // Each list has 3 elements
        Arc::new(values),
        None,
    ))
}

/// Creates a deeply nested struct with multiple levels
pub fn create_deeply_nested_struct(step: i32) -> Arc<dyn Array> {
    let step_f64 = step as f64;

    // Level 3: innermost struct
    let level3_struct = StructArray::from(vec![(
        Arc::new(Field::new("final_value", DataType::Float64, false)),
        Arc::new(Float64Array::from_iter([step_f64 * 1000.0])) as Arc<dyn Array>,
    )]);

    // Level 2: middle struct containing level 3
    let level2_struct = StructArray::from(vec![(
        Arc::new(Field::new(
            "level3",
            DataType::Struct(Fields::from(vec![Field::new(
                "final_value",
                DataType::Float64,
                false,
            )])),
            false,
        )),
        Arc::new(level3_struct) as Arc<dyn Array>,
    )]);

    // Level 1: outer struct containing level 2
    let level1_struct = StructArray::from(vec![(
        Arc::new(Field::new(
            "level2",
            DataType::Struct(Fields::from(vec![Field::new(
                "level3",
                DataType::Struct(Fields::from(vec![Field::new(
                    "final_value",
                    DataType::Float64,
                    false,
                )])),
                false,
            )])),
            false,
        )),
        Arc::new(level2_struct) as Arc<dyn Array>,
    )]);

    // Top level struct
    Arc::new(StructArray::from(vec![
        (
            Arc::new(Field::new("step_value", DataType::Int32, false)),
            Arc::new(Int32Array::from_iter([step])) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new(
                "nested_data",
                DataType::Struct(Fields::from(vec![Field::new(
                    "level2",
                    DataType::Struct(Fields::from(vec![Field::new(
                        "level3",
                        DataType::Struct(Fields::from(vec![Field::new(
                            "final_value",
                            DataType::Float64,
                            false,
                        )])),
                        false,
                    )])),
                    false,
                )])),
                false,
            )),
            Arc::new(level1_struct) as Arc<dyn Array>,
        ),
    ]))
}

/// Creates a struct with optional (nullable) fields
pub fn create_optional_struct(step: i32) -> Arc<dyn Array> {
    Arc::new(StructArray::from(vec![
        (
            Arc::new(Field::new("required_field", DataType::Utf8, false)),
            Arc::new(StringArray::from_iter([Some(format!("step_{step}"))])) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("optional_field", DataType::Utf8, true)),
            Arc::new(StringArray::from_iter([if step % 3 == 0 {
                Some(format!("optional_{step}"))
            } else {
                None
            }])) as Arc<dyn Array>,
        ),
        (
            Arc::new(Field::new("nullable_int", DataType::Int64, true)),
            Arc::new(Int64Array::from_iter([if step % 4 == 0 {
                Some(step as i64 * 100)
            } else {
                None
            }])) as Arc<dyn Array>,
        ),
    ]))
}

pub fn all_arrays() -> Vec<(&'static str, Arc<dyn Array>)> {
    vec![
        ("coordinates_struct", create_coordinates_struct(0)),
        ("metadata_struct", create_metadata_struct(1)),
        ("mixed_lists", create_mixed_lists(2)),
        ("union_array", create_union_array(3)),
        ("simple_map", create_simple_map(4)),
        ("complex_map", create_complex_map(5)),
        ("dictionary_array", create_dictionary_array(6)),
        ("binary_array", create_binary_array(7)),
        ("large_binary_array", create_large_binary_array(8)),
        ("fixed_size_list", create_fixed_size_list(9)),
        ("deeply_nested_struct", create_deeply_nested_struct(10)),
        ("optional_struct", create_optional_struct(11)),
    ]
}
