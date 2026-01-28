mod util;

use std::sync::Arc;

use arrow::{
    array::{Array as _, FixedSizeListArray, Int32Array, ListArray},
    buffer::OffsetBuffer,
    datatypes::{DataType, Field},
};
use re_arrow_combinators::{Selector, SelectorError as Error};
use util::DisplayRB;

use crate::util::fixtures;

#[test]
fn execute_nested_struct() -> Result<(), Error> {
    let array = fixtures::nested_struct_column();

    let result = ".location.x".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable f64] │
    ╞═══════════════════════════════════╡
    │ [1.0]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3.0, 5.0]                        │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null, 7.0]                       │
    └───────────────────────────────────┘
    ");

    Ok(())
}

#[test]
fn execute_identity() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────────────────────┐
    │ col                                               │
    │ ---                                               │
    │ type: nullable List[nullable Struct[1]]           │
    ╞═══════════════════════════════════════════════════╡
    │ [{poses: [{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]}]   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{poses: [{x: 5.0, y: 6.0}]}]                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{poses: []}]                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{poses: [{x: 7.0, y: null}, {x: 9.0, y: 10.0}]}] │
    └───────────────────────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_simple_field() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌──────────────────────────────────────────┐
    │ col                                      │
    │ ---                                      │
    │ type: nullable List[List[Struct[2]]]     │
    ╞══════════════════════════════════════════╡
    │ [[{x: 1.0, y: 2.0}, {x: 3.0, y: 4.0}]]   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 5.0, y: 6.0}]]                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[]]                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[{x: 7.0, y: null}, {x: 9.0, y: 10.0}]] │
    └──────────────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_index() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[0]".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌─────────────────────────────────────────┐
    │ col                                     │
    │ ---                                     │
    │ type: nullable List[nullable Struct[2]] │
    ╞═════════════════════════════════════════╡
    │ [{x: 1.0, y: 2.0}]                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 5.0, y: 6.0}]                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 7.0, y: null}]                     │
    └─────────────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_index_chained() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[0].x".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable f64] │
    ╞═══════════════════════════════════╡
    │ [1.0]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [5.0]                             │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [7.0]                             │
    └───────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_index_to_extract_second_element() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[1]".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌─────────────────────────────────────────┐
    │ col                                     │
    │ ---                                     │
    │ type: nullable List[nullable Struct[2]] │
    ╞═════════════════════════════════════════╡
    │ [{x: 3.0, y: 4.0}]                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [{x: 9.0, y: 10.0}]                     │
    └─────────────────────────────────────────┘
    ");
    Ok(())
}

#[test]
fn execute_array_each() -> Result<(), Error> {
    let array = fixtures::nested_list_struct_column();

    let result = ".poses[]".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
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
    ");
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

    let result = ".poses[10]".parse::<Selector>()?.execute_per_row(&array)?;

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌─────────────────────────────────────────┐
    │ col                                     │
    │ ---                                     │
    │ type: nullable List[nullable Struct[2]] │
    ╞═════════════════════════════════════════╡
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                      │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]                                  │
    └─────────────────────────────────────────┘
    ");
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

    insta::assert_snapshot!(format!("{}", DisplayRB(array.clone())), @"
    ┌──────────────────────────────────────────────────────────────┐
    │ col                                                          │
    │ ---                                                          │
    │ type: nullable List[nullable FixedSizeList[nullable i32; 3]] │
    ╞══════════════════════════════════════════════════════════════╡
    │ [[1, 2, 3], [4, 5, 6]]                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[7, 8, 9]]                                                  │
    └──────────────────────────────────────────────────────────────┘
    ");

    let result = ".[0][1]".parse::<Selector>()?.execute_per_row(&array);

    assert!(matches!(result, Err(Error::Runtime(..))));

    Ok(())
}

#[test]
fn extract_scalar_fields_from_nested_struct() {
    let list_array = fixtures::nested_struct_column();

    let selectors = re_arrow_combinators::extract_nested_fields(&list_array, |dt| {
        matches!(dt, DataType::Float64)
    });

    assert_eq!(selectors[0].to_string(), ".location.x");
    assert_eq!(selectors[1].to_string(), ".location.y");
}
