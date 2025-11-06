#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use re_arrow_combinators::*;

use arrow::{
    array::{
        Array, ArrayRef, Float32Array, Float64Array, Float64Builder, GenericByteBuilder, ListArray,
        ListBuilder, RecordBatch, RecordBatchOptions, StructBuilder,
    },
    datatypes::{DataType, Field, Fields, GenericBinaryType, Schema},
};

/// Helper function to wrap an [`ArrayRef`] into a [`RecordBatch`] for easier printing.
fn wrap_in_record_batch(array: ArrayRef) -> RecordBatch {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("col", array.data_type().clone(), true)],
        Default::default(),
    ));
    RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default()).unwrap()
}

struct DisplayRB<T: Array + Clone + 'static>(T);

impl<T: Array + Clone + 'static> std::fmt::Display for DisplayRB<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rb = wrap_in_record_batch(Arc::new(self.0.clone()));
        write!(f, "{}", re_arrow_util::format_record_batch(&rb))
    }
}

fn create_nasty_component_column() -> ListArray {
    let inner_struct_fields = Fields::from(vec![
        Field::new("x", DataType::Float64, true),
        Field::new("y", DataType::Float64, true),
    ]);

    // Middle struct schema: {poses: List<Struct<x: Float32>>}
    let middle_struct_fields = Fields::from(vec![Field::new(
        "poses",
        DataType::List(Arc::new(Field::new(
            "item",
            DataType::Struct(inner_struct_fields.clone()),
            false,
        ))),
        false,
    )]);

    // Construct nested builders
    let inner_struct_builder = StructBuilder::new(
        inner_struct_fields.clone(),
        vec![
            Box::new(Float64Builder::new()),
            Box::new(Float64Builder::new()),
        ],
    );

    let list_builder = ListBuilder::new(inner_struct_builder).with_field(Arc::new(Field::new(
        "item",
        DataType::Struct(inner_struct_fields),
        false,
    )));

    let struct_builder = StructBuilder::new(middle_struct_fields, vec![Box::new(list_builder)]);

    let mut column_builder = ListBuilder::new(struct_builder);

    // Row 0:
    let struct_val = column_builder.values();
    let list = struct_val
        .field_builder::<ListBuilder<StructBuilder>>(0)
        .unwrap();
    let inner = list.values();
    inner
        .field_builder::<Float64Builder>(0)
        .unwrap()
        .append_value(0.0);
    inner
        .field_builder::<Float64Builder>(1)
        .unwrap()
        .append_value(0.0);
    inner.append(true);
    inner
        .field_builder::<Float64Builder>(0)
        .unwrap()
        .append_value(42.0);
    inner
        .field_builder::<Float64Builder>(1)
        .unwrap()
        .append_value(42.0);
    inner.append(true);
    list.append(true);
    struct_val.append(true);
    column_builder.append(true);

    // Row 1:
    let struct_val = column_builder.values();
    struct_val
        .field_builder::<ListBuilder<StructBuilder>>(0)
        .unwrap()
        .append(true);
    struct_val.append(true);
    column_builder.append(true);

    // Row 2:
    column_builder.append(false);

    // Row 3:
    let struct_val = column_builder.values();
    let list = struct_val
        .field_builder::<ListBuilder<StructBuilder>>(0)
        .unwrap();
    let inner = list.values();
    inner
        .field_builder::<Float64Builder>(0)
        .unwrap()
        .append_value(7.0);
    inner
        .field_builder::<Float64Builder>(1)
        .unwrap()
        .append_null();
    inner.append(true);
    inner
        .field_builder::<Float64Builder>(0)
        .unwrap()
        .append_value(7.0);
    inner
        .field_builder::<Float64Builder>(1)
        .unwrap()
        .append_value(7.0);
    inner.append(true);
    list.append(true);
    struct_val.append(true);
    column_builder.append(true);

    // Row 4:
    let struct_val = column_builder.values();
    let list = struct_val
        .field_builder::<ListBuilder<StructBuilder>>(0)
        .unwrap();
    let inner = list.values();
    inner
        .field_builder::<Float64Builder>(0)
        .unwrap()
        .append_value(17.0);
    inner
        .field_builder::<Float64Builder>(1)
        .unwrap()
        .append_value(17.0);
    inner.append(true);
    list.append(true);
    struct_val.append(true);
    column_builder.append(true);

    column_builder.finish()
}

#[test]
fn simple() {
    let array = create_nasty_component_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = MapList::new(GetField::new("poses"))
        .then(Flatten::new())
        .then(MapList::new(StructToFixedList::new(["x", "y"])));

    let result: ListArray = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!("simple", format!("{}", DisplayRB(result.clone())));
}

#[test]
fn add_one_to_leaves() {
    let array = create_nasty_component_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = MapList::new(GetField::new("poses"))
        .then(Flatten::new())
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(MapPrimitive::<
            arrow::datatypes::Float64Type,
            _,
        >::new(|x| {
            x + 1.0
        }))));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(
        "add_one_to_leaves",
        format!("{}", DisplayRB(result.clone()))
    );
}

#[test]
fn convert_to_f32() {
    let array = create_nasty_component_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = MapList::new(GetField::new("poses"))
        .then(Flatten::new())
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(Cast::<
            Float64Array,
            Float32Array,
        >::new())));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB(result.clone())));
}

#[test]
fn replace_nulls() {
    let array = create_nasty_component_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = MapList::new(GetField::new("poses"))
        .then(Flatten::new())
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(ReplaceNull::<
            arrow::datatypes::Float64Type,
        >::new(1337.0))));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!("replace_nulls", format!("{}", DisplayRB(result.clone())));
}

#[test]
fn test_flatten_single_element() {
    let array = create_nasty_component_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = MapList::new(GetField::new("poses")).then(Flatten::new());

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(
        "flatten_single_element",
        format!("{}", DisplayRB(result.clone()))
    );
}

#[test]
fn test_flatten_multiple_elements() {
    let inner_builder = ListBuilder::new(arrow::array::Int32Builder::new());
    let mut outer_builder = ListBuilder::new(inner_builder);

    // Row 0: [[1, 2], [3, 4]] -> should flatten to [1, 2, 3, 4]
    outer_builder.values().values().append_value(1);
    outer_builder.values().values().append_value(2);
    outer_builder.values().append(true);
    outer_builder.values().values().append_value(3);
    outer_builder.values().values().append_value(4);
    outer_builder.values().append(true);
    outer_builder.append(true);

    // Row 1: [[5, null], [6, 7, 8]] -> should flatten to [5, null, 6, 7, 8]
    outer_builder.values().values().append_value(5);
    outer_builder.values().values().append_null();
    outer_builder.values().append(true);
    outer_builder.values().values().append_value(6);
    outer_builder.values().values().append_value(7);
    outer_builder.values().values().append_value(8);
    outer_builder.values().append(true);
    outer_builder.append(true);

    // Row 2: [[]] -> should flatten to []
    outer_builder.values().append(true);
    outer_builder.append(true);

    // Row 3: [[], [9]] -> should flatten to [9]
    outer_builder.values().append(true);
    outer_builder.values().values().append_value(9);
    outer_builder.values().append(true);
    outer_builder.append(true);

    // Row 4: null -> should remain null
    outer_builder.append(false);

    // Row 5: [[10, 11]] -> should flatten to [10, 11]
    outer_builder.values().values().append_value(10);
    outer_builder.values().values().append_value(11);
    outer_builder.values().append(true);
    outer_builder.append(true);

    // Row 6: [[32], [33, 34], [], null] -> should flatten to [32, 33, 34]
    outer_builder.values().values().append_value(32);
    outer_builder.values().append(true);
    outer_builder.values().values().append_value(33);
    outer_builder.values().values().append_value(34);
    outer_builder.values().append(true);
    outer_builder.values().append(true);
    outer_builder.values().append(false);
    outer_builder.append(true);

    let list_of_lists = outer_builder.finish();

    println!("{}", DisplayRB(list_of_lists.clone()));

    let result = Flatten::new().transform(&list_of_lists).unwrap();

    insta::assert_snapshot!(
        "flatten_multiple_elements",
        format!("{}", DisplayRB(result.clone()))
    );
}

// Generic test for binary arrays where the offset is the same.
fn impl_binary_test<O1: arrow::array::OffsetSizeTrait, O2: arrow::array::OffsetSizeTrait>() {
    println!(
        "Testing '{}' -> '{}'",
        std::any::type_name::<O1>(),
        std::any::type_name::<O2>()
    );

    let mut builder = GenericByteBuilder::<GenericBinaryType<O1>>::new();
    builder.append_value(b"hello");
    builder.append_value(b"world");
    builder.append_null();
    builder.append_value(b"");
    builder.append_value([0x00, 0xFF, 0x42]);
    let binary_array = builder.finish();

    println!("Input:");
    println!("{}", DisplayRB(binary_array.clone()));

    let result = BinaryToListUInt8::<O1, O2>::new()
        .transform(&binary_array)
        .unwrap();

    println!("Output:");
    println!("{}", DisplayRB(result.clone()));

    // Verify structure
    assert_eq!(result.len(), 5);
    assert!(!result.is_null(0));
    assert!(!result.is_null(1));
    assert!(result.is_null(2));
    assert!(!result.is_null(3));
    assert!(!result.is_null(4));

    {
        let list = result.value(0);
        let uint8 = list
            .as_any()
            .downcast_ref::<arrow::array::UInt8Array>()
            .unwrap();
        assert_eq!(uint8.len(), 5);
        assert_eq!(uint8.value(0) as char, 'h');
        assert_eq!(uint8.value(1) as char, 'e');
        assert_eq!(uint8.value(2) as char, 'l');
        assert_eq!(uint8.value(3) as char, 'l');
        assert_eq!(uint8.value(4) as char, 'o');
    }

    {
        let list = result.value(1);
        let uint8 = list
            .as_any()
            .downcast_ref::<arrow::array::UInt8Array>()
            .unwrap();
        assert_eq!(list.len(), 5);
        assert_eq!(uint8.value(0) as char, 'w');
        assert_eq!(uint8.value(1) as char, 'o');
        assert_eq!(uint8.value(2) as char, 'r');
        assert_eq!(uint8.value(3) as char, 'l');
        assert_eq!(uint8.value(4) as char, 'd');
    }

    assert!(result.is_null(2));

    {
        let list = result.value(3);
        let uint8 = list
            .as_any()
            .downcast_ref::<arrow::array::UInt8Array>()
            .unwrap();
        assert_eq!(uint8.len(), 0);
    }

    {
        let list = result.value(4);
        let uint8 = list
            .as_any()
            .downcast_ref::<arrow::array::UInt8Array>()
            .unwrap();
        assert_eq!(uint8.len(), 3);
        assert_eq!(uint8.value(0), 0x00);
        assert_eq!(uint8.value(1), 0xFF);
        assert_eq!(uint8.value(2), 0x42);
    }
}

#[test]
fn test_binary_to_list_uint8() {
    // We test the different offset combinations.
    impl_binary_test::<i32, i32>();
    impl_binary_test::<i64, i32>();
    impl_binary_test::<i32, i64>();
    impl_binary_test::<i64, i64>();
}

#[test]
fn test_binary_offset_overflow() {
    use arrow::array::LargeBinaryArray;
    use arrow::buffer::OffsetBuffer;

    // Create a LargeBinaryArray with an offset that exceeds i32::MAX
    let large_offset = i32::MAX as i64 + 1;

    let offsets = vec![0i64, large_offset];
    let offsets_buffer = OffsetBuffer::new(offsets.into());

    let values = vec![0u8; large_offset as usize];

    let large_binary = LargeBinaryArray::new(offsets_buffer, values.into(), None);

    // Try to convert from LargeBinaryArray (i64 offsets) to ListArray (i32 offsets)
    let transform = BinaryToListUInt8::<i64, i32>::new();
    let result = transform.transform(&large_binary);

    // Should fail with OffsetOverflow
    assert!(result.is_err());
    match result.unwrap_err() {
        Error::OffsetOverflow {
            actual,
            expected_type,
        } => {
            assert_eq!(actual, large_offset as usize);
            assert_eq!(expected_type, "i32");
        }
        other => panic!("Expected OffsetOverflow error, got: {other:?}"),
    }
}
