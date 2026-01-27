mod util;

use std::str::FromStr as _;

use arrow::array::{Float32Array, Float64Array, Int32Builder, ListArray, ListBuilder};
use re_arrow_combinators::Selector;
use re_arrow_combinators::Transform as _;
use re_arrow_combinators::cast::{ListToFixedSizeList, PrimitiveCast};
use re_arrow_combinators::map::{MapFixedSizeList, MapList, MapPrimitive, ReplaceNull};
use re_arrow_combinators::reshape::{RowMajorToColumnMajor, StructToFixedList};
use util::DisplayRB;

use crate::util::fixtures;

#[test]
fn simple() {
    let array = fixtures::nested_list_struct_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = Selector::from_str(".poses[]")
        .unwrap()
        .then(MapList::new(StructToFixedList::new(["x", "y"])));

    let result: ListArray = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result.clone())), @"
    ┌─────────────────────────────────────────────────────┐
    │ col                                                 │
    │ ---                                                 │
    │ type: nullable List[FixedSizeList[nullable f64; 2]] │
    ╞═════════════════════════════════════════════════════╡
    │ [[1.0, 2.0], [3.0, 4.0]]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[5.0, 6.0]]                                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[7.0, null], [9.0, 10.0]]                          │
    └─────────────────────────────────────────────────────┘
    ");
}

#[test]
fn add_one_to_leaves() {
    let array = fixtures::nested_list_struct_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = Selector::from_str(".poses[]")
        .unwrap()
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(MapPrimitive::<
            arrow::datatypes::Float64Type,
            _,
        >::new(|x| {
            x + 1.0
        }))));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(
        format!("{}", DisplayRB(result.clone()))
        , @"
    ┌─────────────────────────────────────────────────────┐
    │ col                                                 │
    │ ---                                                 │
    │ type: nullable List[FixedSizeList[nullable f64; 2]] │
    ╞═════════════════════════════════════════════════════╡
    │ [[2.0, 3.0], [4.0, 5.0]]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[6.0, 7.0]]                                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[8.0, null], [10.0, 11.0]]                         │
    └─────────────────────────────────────────────────────┘
    "
    );
}

#[test]
fn convert_to_f32() {
    let array = fixtures::nested_list_struct_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = Selector::from_str(".poses[]")
        .unwrap()
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(PrimitiveCast::<
            Float64Array,
            Float32Array,
        >::new())));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(DisplayRB(result.clone()), @"
    ┌─────────────────────────────────────────────────────┐
    │ col                                                 │
    │ ---                                                 │
    │ type: nullable List[FixedSizeList[nullable f32; 2]] │
    ╞═════════════════════════════════════════════════════╡
    │ [[1.0, 2.0], [3.0, 4.0]]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[5.0, 6.0]]                                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[7.0, null], [9.0, 10.0]]                          │
    └─────────────────────────────────────────────────────┘
    ");
}

#[test]
fn replace_nulls() {
    let array = fixtures::nested_list_struct_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = Selector::from_str(".poses[]")
        .unwrap()
        .then(MapList::new(StructToFixedList::new(["x", "y"])))
        .then(MapList::new(MapFixedSizeList::new(ReplaceNull::<
            arrow::datatypes::Float64Type,
        >::new(1337.0))));

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result.clone())), @"
    ┌────────────────────────────────────────────┐
    │ col                                        │
    │ ---                                        │
    │ type: nullable List[FixedSizeList[f64; 2]] │
    ╞════════════════════════════════════════════╡
    │ [[1.0, 2.0], [3.0, 4.0]]                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[5.0, 6.0]]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[7.0, 1337.0], [9.0, 10.0]]               │
    └────────────────────────────────────────────┘
    ");
}

#[test]
fn test_flatten_single_element() {
    let array = fixtures::nested_list_struct_column();
    println!("{}", DisplayRB(array.clone()));

    let pipeline = Selector::from_str(".poses[]").unwrap();

    let result = pipeline.transform(&array).unwrap();

    insta::assert_snapshot!(
        format!("{}", DisplayRB(result.clone())), @"
    ┌─────────────────────────────────────────┐
    │ col                                     │
    │ ---                                     │
    │ type: nullable List[nullable Struct[2]] │
    ╞═════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 5.0, y: 6.0}]                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 7.0, y: null}, {x: 9.0, y: 10.0}]  │
    └─────────────────────────────────────────┘
    "
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

    let result = Selector::from_str(".[]")
        .unwrap()
        .transform(&list_of_lists)
        .unwrap();

    insta::assert_snapshot!(
        format!("{}", DisplayRB(result.clone())), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [1, 2, 3, 4]                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [5, null, 6, 7, 8]                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [9]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [10, 11]                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [32, 33, 34]                      │
    └───────────────────────────────────┘
    "
    );
}

#[test]
fn test_row_major_to_col_major() {
    let inner_builder = Int32Builder::new();
    let mut outer_builder = ListBuilder::new(inner_builder);

    // First list represents a 4x3 matrix in row-major order with some null elements.
    // Row 0
    outer_builder.values().append_value(1);
    outer_builder.values().append_null();
    outer_builder.values().append_value(3);
    // Row 1
    outer_builder.values().append_value(4);
    outer_builder.values().append_value(5);
    outer_builder.values().append_value(6);
    // Row 2
    outer_builder.values().append_value(7);
    outer_builder.values().append_value(8);
    outer_builder.values().append_null();
    // Row 3
    outer_builder.values().append_value(10);
    outer_builder.values().append_value(11);
    outer_builder.values().append_value(12);
    outer_builder.append(true);

    // Second list is invalid / null.
    for _ in 0..12 {
        // Add dummy values for Arrow's fixed-size requirements.
        // See: https://docs.rs/arrow/latest/arrow/array/struct.FixedSizeListArray.html#representation
        outer_builder.values().append_value(0);
    }
    outer_builder.append(false);

    // Third list represents a 4x3 matrix in row-major order without null elements.
    // Row 0
    outer_builder.values().append_value(13);
    outer_builder.values().append_value(14);
    outer_builder.values().append_value(15);
    // Row 1
    outer_builder.values().append_value(16);
    outer_builder.values().append_value(17);
    outer_builder.values().append_value(18);
    // Row 2
    outer_builder.values().append_value(19);
    outer_builder.values().append_value(20);
    outer_builder.values().append_value(21);
    // Row 3
    outer_builder.values().append_value(22);
    outer_builder.values().append_value(23);
    outer_builder.values().append_value(24);
    outer_builder.append(true);

    let input_array = outer_builder.finish();

    // Cast to `FixedSizeListArray` and convert to column-major order.
    let fixed_size_list_array = ListToFixedSizeList::new(12)
        .transform(&input_array)
        .unwrap();
    let result = RowMajorToColumnMajor::new(4, 3)
        .transform(&fixed_size_list_array)
        .unwrap();

    insta::assert_snapshot!(
        format!("{}", DisplayRB(result.clone())), @"
    ┌──────────────────────────────────────────────────┐
    │ col                                              │
    │ ---                                              │
    │ type: nullable FixedSizeList[nullable i32; 12]   │
    ╞══════════════════════════════════════════════════╡
    │ [1, 4, 7, 10, null, 5, 8, 11, 3, 6, null, 12]    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [13, 16, 19, 22, 14, 17, 20, 23, 15, 18, 21, 24] │
    └──────────────────────────────────────────────────┘
    "
    );
}
