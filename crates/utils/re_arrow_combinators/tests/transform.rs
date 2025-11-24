#![expect(clippy::unwrap_used)]

mod util;

use std::sync::Arc;

use arrow::{
    array::{Float32Array, Float64Array, Float64Builder, ListArray, ListBuilder, StructBuilder},
    datatypes::{DataType, Field, Fields},
};
use re_arrow_combinators::{
    Transform as _,
    cast::PrimitiveCast,
    map::{MapFixedSizeList, MapList, MapPrimitive, ReplaceNull},
    reshape::{Flatten, GetField, StructToFixedList},
};
use util::DisplayRB;

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
        .then(MapList::new(MapFixedSizeList::new(PrimitiveCast::<
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
