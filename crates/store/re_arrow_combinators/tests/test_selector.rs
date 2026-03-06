mod util;

use std::sync::Arc;

use arrow::{
    array::{Array as _, FixedSizeListArray, Int32Array, ListArray},
    buffer::OffsetBuffer,
    datatypes::{DataType, Field, Fields},
};
use re_arrow_combinators::{Selector, SelectorError as Error};
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
        .execute_per_row(&array);

    assert!(matches!(
        result,
        Err(Error::Runtime(
            re_arrow_combinators::Error::FieldNotFound { .. }
        ))
    ));
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

    // Without `?`, accessing a field that doesn't exist errors.
    let err = ".location.z".parse::<Selector>()?.execute_per_row(&array);
    assert!(matches!(
        err,
        Err(Error::Runtime(
            re_arrow_combinators::Error::FieldNotFound { .. }
        ))
    ));
    let err = ".foo.x".parse::<Selector>()?.execute_per_row(&array);
    assert!(matches!(
        err,
        Err(Error::Runtime(
            re_arrow_combinators::Error::FieldNotFound { .. }
        ))
    ),);

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
        Err(Error::Runtime(
            re_arrow_combinators::Error::TypeMismatch { .. }
        ))
    ));

    // With `?`, the error is suppressed and we get `None`.
    let result = ".[]?".parse::<Selector>()?.execute_per_row(&array)?;
    assert!(
        result.is_none(),
        "optional each should return None on non-list inner type"
    );

    Ok(())
}

fn formatted(pair: impl IntoIterator<Item = (Selector, DataType)>) -> String {
    pair.into_iter()
        .map(|(sel, dt)| format!("{sel} ({dt})"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn extract_scalar_fields_from_nested_struct() {
    // Schema:
    //   ┌─ a (struct)
    //   │  ├─ b: Float64
    //   │  └─ c: Int32
    //   └─ d: Int32

    let bc_fields = Fields::from(vec![
        Field::new("b", DataType::Float64, true),
        Field::new("c", DataType::Int32, true),
    ]);

    let e_field = Field::new_list_field(DataType::Float32, false);

    let root_fields = Fields::from(vec![
        Field::new("a", DataType::Struct(bc_fields), true),
        Field::new("d", DataType::Int32, true),
        Field::new("e", DataType::FixedSizeList(e_field.into(), 3), true),
    ]);

    let datatype = DataType::Struct(root_fields);

    let result = re_arrow_combinators::extract_nested_fields(&datatype, |dt| {
        matches!(dt, DataType::Float64 | DataType::Float32 | DataType::Int32)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .d (Int32)
    .e[] (Float32)
    .a.b (Float64)
    .a.c (Int32)
    ");
}

#[test]
fn extract_scalar_fields_from_nested_list_struct() {
    // Schema:
    //   ┌─ a (struct)
    //   │  ├─ b: [Float64]
    //   │  └─ c: [Int32]
    //   └─ d: [Float64]

    let b_list = DataType::List(Field::new_list_field(DataType::Float64, true).into());
    let c_list = DataType::List(Field::new_list_field(DataType::Int32, true).into());
    let bc_fields = Fields::from(vec![
        Field::new("b", b_list, true),
        Field::new("c", c_list, true),
    ]);

    let d_list = DataType::List(Field::new_list_field(DataType::Float64, true).into());
    let e_list = DataType::List(
        Field::new_list_field(
            DataType::FixedSizeList(Field::new_list_field(DataType::Float32, false).into(), 3),
            true,
        )
        .into(),
    );
    let root_fields = Fields::from(vec![
        Field::new("a", DataType::Struct(bc_fields), true),
        Field::new("d", d_list, true),
        Field::new("e", e_list, true),
    ]);

    let datatype = DataType::Struct(root_fields);

    let result = re_arrow_combinators::extract_nested_fields(&datatype, |dt| {
        matches!(dt, DataType::Float64 | DataType::Float32 | DataType::Int32)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .d[] (Float64)
    .e[][] (Float32)
    .a.b[] (Float64)
    .a.c[] (Int32)
    ");
}

#[test]
fn extract_nested_fields_fixtures() {
    let array = fixtures::nested_struct_column();
    let result = re_arrow_combinators::extract_nested_fields(&array.value_type(), |dt| {
        matches!(dt, DataType::Float64)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .location.x (Float64)
    .location.y (Float64)
    ");

    let array = fixtures::nested_list_struct_column();
    let result = re_arrow_combinators::extract_nested_fields(&array.value_type(), |dt| {
        matches!(dt, DataType::Float64)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .poses[].x (Float64)
    .poses[].y (Float64)
    ");
}
