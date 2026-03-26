// This is fine, since this is only used in tests.
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field, Schema};

/// Helper function to wrap an [`ArrayRef`] into a [`RecordBatch`] for easier printing.
fn wrap_in_record_batch(array: ArrayRef) -> RecordBatch {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("col", array.data_type().clone(), true)],
        Default::default(),
    ));
    RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default()).unwrap()
}

pub struct DisplayRB<T: Array + Clone + 'static>(pub T);

impl<T: Array + Clone + 'static> std::fmt::Display for DisplayRB<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rb = wrap_in_record_batch(Arc::new(self.0.clone()));
        write!(f, "{}", re_arrow_util::format_record_batch(&rb))
    }
}

pub mod fixtures {
    use std::sync::Arc;

    use arrow::{
        array::{
            Array as _, ArrayData, Float64Array, ListArray, StringArray, StructArray, UInt8Array,
        },
        buffer::{Buffer, NullBuffer, OffsetBuffer},
        datatypes::{DataType, Field, Fields},
    };

    fn shared_values_array() -> Arc<Float64Array> {
        Arc::new(Float64Array::from(vec![
            1.0, 0.0, 3.0, 5.0, 0.0, 7.0, 2.0, 0.0, 4.0, 6.0, 0.0, 8.0, 1.0, 3.0, 5.0, 7.0, 9.0,
            2.0, 4.0, 6.0, 0.0, 10.0,
        ]))
    }

    #[test]
    fn example_nested_struct_column() {
        let array = nested_struct_column();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r#"
        ┌────────────────────────────────────────────────────────────────────┐
        │ col                                                                │
        │ ---                                                                │
        │ type: List(Struct("location": Struct("x": Float64, "y": Float64))) │
        ╞════════════════════════════════════════════════════════════════════╡
        │ [{location: {x: 1.0, y: 2.0}}]                                     │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{location: null}]                                                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ []                                                                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                                                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{location: {x: 3.0, y: 4.0}}, {location: {x: 5.0, y: 6.0}}]       │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null, {location: {x: 7.0, y: 8.0}}]                               │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{location: null}, {location: null}]                               │
        └────────────────────────────────────────────────────────────────────┘
        "#);
    }

    pub fn nested_struct_column() -> ListArray {
        let values = shared_values_array();
        let values_buffer = values.to_data().buffers()[0].clone();

        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, true),
            Field::new("y", DataType::Float64, true),
        ]);

        let x_nulls = NullBuffer::from(vec![true, false, true, true, false, true, false, false]);
        let y_nulls = NullBuffer::from(vec![true, false, true, true, false, true, false, false]);

        let x_data = ArrayData::builder(DataType::Float64)
            .len(8)
            .null_bit_buffer(Some(x_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(0, 64))
            .build()
            .unwrap();

        let y_data = ArrayData::builder(DataType::Float64)
            .len(8)
            .null_bit_buffer(Some(y_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(48, 64))
            .build()
            .unwrap();

        let x_array = Float64Array::from(x_data);
        let y_array = Float64Array::from(y_data);

        let inner_struct_nulls =
            NullBuffer::from(vec![true, false, true, true, false, true, false, false]);
        let inner_struct = StructArray::new(
            inner_struct_fields.clone(),
            vec![Arc::new(x_array), Arc::new(y_array)],
            Some(inner_struct_nulls),
        );

        let outer_struct_fields = Fields::from(vec![Field::new(
            "location",
            DataType::Struct(inner_struct_fields),
            true,
        )]);

        let outer_struct_nulls =
            NullBuffer::from(vec![true, true, true, true, false, true, true, true]);
        let outer_struct = StructArray::new(
            outer_struct_fields,
            vec![Arc::new(inner_struct)],
            Some(outer_struct_nulls),
        );

        let list_offsets = OffsetBuffer::from_lengths([1, 1, 0, 0, 2, 2, 2]);

        let list_nulls = NullBuffer::from(vec![true, true, true, false, true, true, true]);

        ListArray::new(
            Arc::new(Field::new_list_field(
                DataType::Struct(outer_struct.fields().clone()),
                true,
            )),
            list_offsets,
            Arc::new(outer_struct),
            Some(list_nulls),
        )
    }

    // TODO(grtlr): Make a second Pose struct!
    #[test]
    fn example_nested_list_struct_column() {
        let array = nested_list_struct_column();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r#"
        ┌─────────────────────────────────────────────────────────────────────────────────────────┐
        │ col                                                                                     │
        │ ---                                                                                     │
        │ type: List(Struct("poses": non-null List(non-null Struct("x": Float64, "y": Float64)))) │
        ╞═════════════════════════════════════════════════════════════════════════════════════════╡
        │ [{poses: [{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]}]                                         │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{poses: [{x: 5.0, y: 6.0}]}]                                                           │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{poses: []}]                                                                           │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ []                                                                                      │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                                                                                    │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{poses: [{x: 7.0, y: null}, {x: 9.0, y: 10.0}]}]                                       │
        └─────────────────────────────────────────────────────────────────────────────────────────┘
        "#);
    }

    pub fn nested_list_struct_column() -> ListArray {
        let values = shared_values_array();
        let values_buffer = values.to_data().buffers()[0].clone();

        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, true),
            Field::new("y", DataType::Float64, true),
        ]);

        let x_nulls = NullBuffer::from(vec![true, true, true, true, true]);
        let y_nulls = NullBuffer::from(vec![true, true, true, false, true]);

        let x_data = ArrayData::builder(DataType::Float64)
            .len(5)
            .null_bit_buffer(Some(x_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(96, 40))
            .build()
            .unwrap();

        let y_data = ArrayData::builder(DataType::Float64)
            .len(5)
            .null_bit_buffer(Some(y_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(136, 40))
            .build()
            .unwrap();

        let x_array = Float64Array::from(x_data);
        let y_array = Float64Array::from(y_data);

        let inner_struct = StructArray::new(
            inner_struct_fields.clone(),
            vec![Arc::new(x_array), Arc::new(y_array)],
            None,
        );

        let poses_offsets = OffsetBuffer::from_lengths([2, 1, 0, 2]);

        let poses_list = ListArray::new(
            Arc::new(Field::new_list_field(
                DataType::Struct(inner_struct_fields.clone()),
                false,
            )),
            poses_offsets,
            Arc::new(inner_struct),
            None,
        );

        let middle_struct_fields = Fields::from(vec![Field::new(
            "poses",
            DataType::List(Arc::new(Field::new_list_field(
                DataType::Struct(inner_struct_fields),
                false,
            ))),
            false,
        )]);

        let middle_struct = StructArray::new(
            middle_struct_fields.clone(),
            vec![Arc::new(poses_list)],
            None,
        );

        let outer_offsets = OffsetBuffer::from_lengths([1, 1, 1, 0, 0, 1]);

        let outer_nulls = NullBuffer::from(vec![true, true, true, true, false, true]);

        ListArray::new(
            Arc::new(Field::new_list_field(
                DataType::Struct(middle_struct_fields),
                true,
            )),
            outer_offsets,
            Arc::new(middle_struct),
            Some(outer_nulls),
        )
    }

    #[test]
    fn example_nested_string_struct_column() {
        let list_array = nested_string_struct_column();
        insta::assert_snapshot!(super::DisplayRB(list_array.clone()), @r#"
        ┌───────────────────────────────────────────────────────────────────┐
        │ col                                                               │
        │ ---                                                               │
        │ type: List(Struct("data": Struct("names": Utf8, "colors": Utf8))) │
        ╞═══════════════════════════════════════════════════════════════════╡
        │ [{data: {names: alice, colors: red}}]                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [null]                                                            │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                                                              │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [{data: null}, {data: {names: dave, colors: yellow}}]             │
        └───────────────────────────────────────────────────────────────────┘
        "#);
    }

    /// Creates a nested struct column with string values from an underlying shared buffer.
    pub fn nested_string_struct_column() -> ListArray {
        // Create a shared StringArray containing all data.
        let shared_strings = StringArray::from(vec![
            "alice", "bob", "carol", "dave", // names
            "red", "green", "blue", "yellow", // colors
        ]);

        // Extract the raw values buffer (the UTF-8 bytes).
        let values_buffer = shared_strings.to_data().buffers()[1].clone();

        // Create "names" StringArray using first part of shared buffer.
        // Strings: "alice" (0-5), "bob" (5-8), "carol" (8-13), "dave" (13-17)
        let names_offsets: &[i32] = &[0, 5, 8, 13, 17];
        let names_nulls = NullBuffer::from(vec![true, false, true, true]); // set "bob" as null

        let names_data = ArrayData::builder(DataType::Utf8)
            .len(4)
            .null_bit_buffer(Some(names_nulls.buffer().clone()))
            .add_buffer(Buffer::from_slice_ref(names_offsets))
            .add_buffer(values_buffer.slice_with_length(0, 17))
            .build()
            .unwrap();
        let names_array = StringArray::from(names_data);

        // Create "colors" StringArray using second part of shared buffer.
        let offset_start = names_offsets.last().copied().unwrap() as usize;
        // Strings relative to slice: "red" (0-3), "green" (3-8), "blue" (8-12), "yellow" (12-18)
        let colors_offsets: &[i32] = &[0, 3, 8, 12, 18];
        let colors_nulls = NullBuffer::from(vec![true, true, false, true]); // set "blue" as null

        let colors_data = ArrayData::builder(DataType::Utf8)
            .len(4)
            .null_bit_buffer(Some(colors_nulls.buffer().clone()))
            .add_buffer(Buffer::from_slice_ref(colors_offsets))
            .add_buffer(values_buffer.slice_with_length(offset_start, 18))
            .build()
            .unwrap();
        let colors_array = StringArray::from(colors_data);

        // Build the inner struct containing both string arrays.
        let inner_struct_fields = Fields::from(vec![
            Field::new("names", DataType::Utf8, true),
            Field::new("colors", DataType::Utf8, true),
        ]);
        let inner_struct_nulls = NullBuffer::from(vec![true, true, false, true]); // third element is null
        let inner_struct = StructArray::new(
            inner_struct_fields.clone(),
            vec![
                Arc::new(names_array.clone()),
                Arc::new(colors_array.clone()),
            ],
            Some(inner_struct_nulls),
        );

        // Wrap in an outer struct with a "data" field.
        let outer_struct_fields = Fields::from(vec![Field::new(
            "data",
            DataType::Struct(inner_struct_fields.clone()),
            true,
        )]);
        let outer_struct_nulls = NullBuffer::from(vec![true, false, true, true]); // second element is null
        let outer_struct = StructArray::new(
            outer_struct_fields.clone(),
            vec![Arc::new(inner_struct)],
            Some(outer_struct_nulls),
        );

        // Wrap in a ListArray.
        use arrow::buffer::OffsetBuffer;
        let list_offsets = OffsetBuffer::from_lengths([1, 1, 0, 2]); // varying list lengths
        let list_nulls = NullBuffer::from(vec![true, true, false, true]); // third list is null

        arrow::array::ListArray::new(
            Arc::new(Field::new_list_field(
                DataType::Struct(outer_struct_fields),
                true,
            )),
            list_offsets,
            Arc::new(outer_struct),
            Some(list_nulls),
        )
    }

    #[test]
    fn example_struct_column() {
        let array = struct_column();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r#"
        ┌──────────────────────────────────────────────────────────────┐
        │ col                                                          │
        │ ---                                                          │
        │ type: Struct("location": Struct("x": Float64, "y": Float64)) │
        ╞══════════════════════════════════════════════════════════════╡
        │ {location: {x: 1.0, y: 2.0}}                                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: null}                                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: {x: 3.0, y: 4.0}}                                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: {x: 5.0, y: null}}                                │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                                                         │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: {x: 7.0, y: 8.0}}                                 │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: null}                                             │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ {location: null}                                             │
        └──────────────────────────────────────────────────────────────┘
        "#);
    }

    /// A plain `StructArray` (not wrapped in a `ListArray`) with a single `"location"` field
    /// containing `Struct { x: Float64, y: Float64 }`.
    ///
    /// This is the same as the inner data of [`nested_struct_column`] before it gets wrapped
    /// in a `ListArray`.
    pub fn struct_column() -> StructArray {
        let values = shared_values_array();
        let values_buffer = values.to_data().buffers()[0].clone();

        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, true),
            Field::new("y", DataType::Float64, true),
        ]);

        let x_nulls = NullBuffer::from(vec![true, false, true, true, false, true, false, false]);
        let y_nulls = NullBuffer::from(vec![true, false, true, false, false, true, false, false]);

        let x_data = ArrayData::builder(DataType::Float64)
            .len(8)
            .null_bit_buffer(Some(x_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(0, 64))
            .build()
            .unwrap();

        let y_data = ArrayData::builder(DataType::Float64)
            .len(8)
            .null_bit_buffer(Some(y_nulls.buffer().clone()))
            .add_buffer(values_buffer.slice_with_length(48, 64))
            .build()
            .unwrap();

        let x_array = Float64Array::from(x_data);
        let y_array = Float64Array::from(y_data);

        let inner_struct_nulls =
            NullBuffer::from(vec![true, false, true, true, false, true, false, false]);
        let inner_struct = StructArray::new(
            inner_struct_fields.clone(),
            vec![Arc::new(x_array), Arc::new(y_array)],
            Some(inner_struct_nulls),
        );

        let outer_struct_fields = Fields::from(vec![Field::new(
            "location",
            DataType::Struct(inner_struct_fields),
            true,
        )]);

        let outer_struct_nulls =
            NullBuffer::from(vec![true, true, true, true, false, true, true, true]);
        StructArray::new(
            outer_struct_fields,
            vec![Arc::new(inner_struct)],
            Some(outer_struct_nulls),
        )
    }

    #[test]
    fn example_list_not_nullable() {
        let array = list_not_nullable();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r"
        ┌────────────────────────────┐
        │ col                        │
        │ ---                        │
        │ type: List(non-null UInt8) │
        ╞════════════════════════════╡
        │ [1, 2]                     │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [3, 4, 5]                  │
        └────────────────────────────┘
        ");
    }

    pub fn list_not_nullable() -> ListArray {
        let values = UInt8Array::from(vec![1, 2, 3, 4, 5]);
        let offsets = OffsetBuffer::from_lengths([2, 3]);

        ListArray::new(
            Arc::new(Field::new_list_field(DataType::UInt8, false)),
            offsets,
            Arc::new(values),
            None,
        )
    }

    #[test]
    fn example_list_with_nulls() {
        let array = list_with_nulls();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r"
        ┌───────────────────┐
        │ col               │
        │ ---               │
        │ type: List(UInt8) │
        ╞═══════════════════╡
        │ [1, 2]            │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null              │
        └───────────────────┘
        ");
    }

    pub fn list_with_nulls() -> ListArray {
        let values = UInt8Array::from(vec![1, 2, 3, 4, 5]);
        let offsets = OffsetBuffer::from_lengths([2, 3]);
        let nulls = NullBuffer::from(vec![true, false]);

        ListArray::new(
            Arc::new(Field::new_list_field(DataType::UInt8, true)),
            offsets,
            Arc::new(values),
            Some(nulls),
        )
    }

    #[test]
    fn example_deep_nested_list_column() {
        let array = deep_nested_list_column();
        insta::assert_snapshot!(format!("{}", super::DisplayRB(array)), @r#"
        ┌─────────────────────────────────┐
        │ col                             │
        │ ---                             │
        │ type: List(List(List(Float64))) │
        ╞═════════════════════════════════╡
        │ [[[1.0, 3.0], [5.0]], [[7.0]]]  │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [[[]]]                          │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [[]]                            │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ null                            │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ []                              │
        ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
        │ [[[9.0]]]                       │
        └─────────────────────────────────┘
        "#);
    }

    pub fn deep_nested_list_column() -> ListArray {
        let values = shared_values_array();
        let values_buffer = values.to_data().buffers()[0].clone();

        let float_data = ArrayData::builder(DataType::Float64)
            .len(5)
            .add_buffer(values_buffer.slice_with_length(12 * 8, 5 * 8))
            .build()
            .unwrap();
        let float_values = Float64Array::from(float_data);

        let inner_field = Arc::new(Field::new_list_field(DataType::Float64, true));
        let inner_offsets = OffsetBuffer::from_lengths([2, 1, 1, 0, 1]);
        let inner_list = ListArray::new(inner_field, inner_offsets, Arc::new(float_values), None);

        let middle_field = Arc::new(Field::new_list_field(
            DataType::List(Arc::new(Field::new_list_field(DataType::Float64, true))),
            true,
        ));
        let middle_offsets = OffsetBuffer::from_lengths([2, 1, 1, 0, 1]);
        let middle_list = ListArray::new(middle_field, middle_offsets, Arc::new(inner_list), None);

        // Outer ListArray (6 rows): lengths [2, 1, 1, 0, 0, 1], row 3 is null
        let outer_field = Arc::new(Field::new_list_field(
            DataType::List(Arc::new(Field::new_list_field(
                DataType::List(Arc::new(Field::new_list_field(DataType::Float64, true))),
                true,
            ))),
            true,
        ));
        let outer_offsets = OffsetBuffer::from_lengths([2, 1, 1, 0, 0, 1]);
        let outer_nulls = NullBuffer::from(vec![true, true, true, false, true, true]);

        ListArray::new(
            outer_field,
            outer_offsets,
            Arc::new(middle_list),
            Some(outer_nulls),
        )
    }
}
