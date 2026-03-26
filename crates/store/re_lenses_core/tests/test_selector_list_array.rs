mod util;

use std::sync::Arc;

use arrow::{
    array::{Array as _, FixedSizeListArray, Int32Array, ListArray},
    buffer::OffsetBuffer,
    datatypes::{DataType, Field},
    error::ArrowError,
};
use re_lenses_core::{Selector, SelectorError as Error};
use util::DisplayRB;

use crate::util::fixtures;

#[test]
fn execute_nested_struct() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    let result = ".location.x"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3.0, 5.0]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, 7.0]         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]        │
    └─────────────────────┘
    ");

    Ok(())
}

#[test]
fn execute_identity() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".".parse::<Selector>()?.execute_per_row(&array)?.unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
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
    Ok(())
}

#[test]
fn execute_simple_field() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌───────────────────────────────────────────────────────────────┐
    │ col                                                           │
    │ ---                                                           │
    │ type: List(List(non-null Struct("x": Float64, "y": Float64))) │
    ╞═══════════════════════════════════════════════════════════════╡
    │ [[{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]]                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 5.0, y: 6.0}]]                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[]]                                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 7.0, y: null}, {x: 9.0, y: 10.0}]]                      │
    └───────────────────────────────────────────────────────────────┘
    "#);

    let result = "map(.poses)"
        .parse::<Selector>()?
        .execute(Arc::new(array.clone()))?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌───────────────────────────────────────────────────────────────┐
    │ col                                                           │
    │ ---                                                           │
    │ type: List(List(non-null Struct("x": Float64, "y": Float64))) │
    ╞═══════════════════════════════════════════════════════════════╡
    │ [[{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]]                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 5.0, y: 6.0}]]                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[]]                                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                                          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 7.0, y: null}, {x: 9.0, y: 10.0}]]                      │
    └───────────────────────────────────────────────────────────────┘
    "#);

    Ok(())
}

#[test]
fn execute_index() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[0]"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 5.0, y: 6.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 7.0, y: null}]                            │
    └────────────────────────────────────────────────┘
    "#);
    Ok(())
}

#[test]
fn execute_index_chained() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[0].x"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [5.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [7.0]               │
    └─────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_index_to_extract_second_element() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[1]"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 3.0, y: 4.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 9.0, y: 10.0}]                            │
    └────────────────────────────────────────────────┘
    "#);
    Ok(())
}

#[test]
fn execute_array_each() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[]"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 5.0, y: 6.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 7.0, y: null}, {x: 9.0, y: 10.0}]         │
    └────────────────────────────────────────────────┘
    "#);
    Ok(())
}

#[test]
fn execute_parse_error() {
    let result = ".poses[".parse::<Selector>();

    assert!(matches!(result, Err(Error::Parse(_))));
}

#[test]
fn execute_missing_field() {
    let array = fixtures::nested_list_struct_column();

    let result = ".nonexistent"
        .parse::<Selector>()
        .unwrap()
        .execute_per_row(&array)
        .expect("should not error");

    assert!(result.is_none(), "missing field should return None");
}

#[test]
fn execute_index_out_of_bounds() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[10]"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    └────────────────────────────────────────────────┘
    "#);
    Ok(())
}

// TODO(RR-3435): Implement indexing into `FixedSizeListArray`.
#[test]
fn execute_index_on_fixed_size_list() -> Result<(), Error> {
    let values = Int32Array::from(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let fixed_field = Arc::new(Field::new("item", DataType::Int32, true));
    let fixed_list = FixedSizeListArray::new(fixed_field, 3, Arc::new(values), None);

    let offsets = OffsetBuffer::new(vec![0, 2, 3].into());
    let list_field = Arc::new(Field::new_list_field(fixed_list.data_type().clone(), true));
    let array = ListArray::new(list_field, offsets, Arc::new(fixed_list), None);

    insta::assert_snapshot!(format!("{}", DisplayRB(array.clone())), @r"
    ┌──────────────────────────────────────┐
    │ col                                  │
    │ ---                                  │
    │ type: List(FixedSizeList(3 x Int32)) │
    ╞══════════════════════════════════════╡
    │ [[1, 2, 3], [4, 5, 6]]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[7, 8, 9]]                          │
    └──────────────────────────────────────┘
    ");

    let result = ".[0][1]".parse::<Selector>()?.execute_per_row(&array);

    assert!(matches!(result, Err(Error::Runtime(..))));

    Ok(())
}

#[test]
fn execute_each_on_fixed_size_list() -> Result<(), Error> {
    // Build List<FixedSizeList<Int32, 3>>
    //   Row 0: [[1,2,3], [4,5,6]]  -> flatten to [1,2,3,4,5,6]
    //   Row 1: [[7,8,9]]           -> flatten to [7,8,9]

    let values = Int32Array::from(vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let fixed_field = Arc::new(Field::new("item", DataType::Int32, true));
    let fixed_list = FixedSizeListArray::new(fixed_field, 3, Arc::new(values), None);

    let offsets = OffsetBuffer::new(vec![0, 2, 3].into());
    let list_field = Arc::new(Field::new_list_field(fixed_list.data_type().clone(), true));
    let array = ListArray::new(list_field, offsets, Arc::new(fixed_list), None);

    let result = ".[]".parse::<Selector>()?.execute_per_row(&array)?.unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌────────────────────┐
    │ col                │
    │ ---                │
    │ type: List(Int32)  │
    ╞════════════════════╡
    │ [1, 2, 3, 4, 5, 6] │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [7, 8, 9]          │
    └────────────────────┘
    ");

    Ok(())
}

#[test]
fn execute_optional_field() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    // Accessing a field that doesn't exist returns `None`.
    let result = ".location.z".parse::<Selector>()?.execute_per_row(&array)?;
    assert!(result.is_none(), "missing field should return None");

    let result = ".foo.x".parse::<Selector>()?.execute_per_row(&array)?;
    assert!(result.is_none(), "missing field should return None");

    // With `?`, the missing field is suppressed and we get `None` instead.
    let result = ".location.z?"
        .parse::<Selector>()?
        .execute_per_row(&array)?;

    assert!(result.is_none(), "optional segment should return None");
    let result = ".foo?.x".parse::<Selector>()?.execute_per_row(&array)?;

    assert!(result.is_none(), "optional segment should return None");

    Ok(())
}

#[test]
fn execute_optional_each_suppressed() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    // Without `?`, `[]` on a struct (non-list) inner type errors.
    let err = ".[]".parse::<Selector>()?.execute_per_row(&array);
    assert!(matches!(
        err,
        Err(Error::Runtime(re_lenses_core::combinators::Error::Arrow(ref e)))
            if matches!(e.as_ref(), ArrowError::InvalidArgumentError(..))
    ));

    // With `?`, the error is suppressed and we get `None`.
    let result = ".[]?".parse::<Selector>()?.execute_per_row(&array)?;
    assert!(
        result.is_none(),
        "optional each should return None on non-list inner type"
    );

    Ok(())
}

#[test]
fn execute_non_null_field() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    // Without `!`, row 1 is `[null]` (inner null within a list)
    let without = ".location"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(without)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 3.0, y: 4.0}, {x: 5.0, y: 6.0}]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, {x: 7.0, y: 8.0}]                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]                                   │
    └────────────────────────────────────────────────┘
    "#);

    // With `!`, all-null rows ([null] and [null, null]) are promoted to outer nulls
    let result = ".location!"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 3.0, y: 4.0}, {x: 5.0, y: 6.0}]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, {x: 7.0, y: 8.0}]                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    └────────────────────────────────────────────────┘
    "#);

    Ok(())
}

#[test]
fn execute_non_null_nested() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    // Without `!`, row 1 is `[null]` (inner null within a list)
    let without = ".location"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(without)), @r#"
    ┌────────────────────────────────────────────────┐
    │ col                                            │
    │ ---                                            │
    │ type: List(Struct("x": Float64, "y": Float64)) │
    ╞════════════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 3.0, y: 4.0}, {x: 5.0, y: 6.0}]           │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, {x: 7.0, y: 8.0}]                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, null]                                   │
    └────────────────────────────────────────────────┘
    "#);

    // With `!` on the intermediate field, null locations are promoted before accessing `.x`
    let result = ".location!.x"
        .parse::<Selector>()?
        .execute_per_row(&array)?
        .unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3.0, 5.0]          │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, 7.0]         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                │
    └─────────────────────┘
    ");

    Ok(())
}
