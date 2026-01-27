mod util;

use std::sync::Arc;

use arrow::array::{Array as _, Int32Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Int32Type};
use re_arrow_combinators::Transform as _;
use re_arrow_combinators::reshape::Explode;
use util::DisplayRB;

#[test]
fn test_explode_primitives() {
    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2), Some(3)]),
        Some(vec![Some(4), Some(5)]),
        Some(vec![Some(6)]),
    ]);

    insta::assert_snapshot!(format!("{}", DisplayRB(input.clone())), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [1, 2, 3]                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [4, 5]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [6]                               │
    └───────────────────────────────────┘
    ");

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [1]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [2]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [4]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [5]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [6]                               │
    └───────────────────────────────────┘
    ");
}

#[test]
fn test_explode_with_nulls_and_empty() {
    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        None,
        Some(vec![]),
        Some(vec![Some(3)]),
    ]);

    insta::assert_snapshot!(format!("{}", DisplayRB(input.clone())), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [1, 2]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3]                               │
    └───────────────────────────────────┘
    ");

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [1]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [2]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ []                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3]                               │
    └───────────────────────────────────┘
    ");
}

#[test]
fn test_explode_nested_lists() {
    let inner_values = Int32Array::from(vec![1, 2, 3, 4, 5, 6]);
    let inner_offsets = OffsetBuffer::new(vec![0, 2, 3, 6].into());
    let inner_field = Arc::new(Field::new_list_field(DataType::Int32, true));
    let inner_list = ListArray::new(inner_field, inner_offsets, Arc::new(inner_values), None);

    let outer_offsets = OffsetBuffer::new(vec![0, 2, 3].into());
    let outer_field = Arc::new(Field::new_list_field(inner_list.data_type().clone(), true));
    let input = ListArray::new(
        outer_field,
        outer_offsets,
        Arc::new(inner_list.clone()),
        None,
    );

    insta::assert_snapshot!(format!("{}", DisplayRB(input.clone())), @"
    ┌──────────────────────────────────────────────────┐
    │ col                                              │
    │ ---                                              │
    │ type: nullable List[nullable List[nullable i32]] │
    ╞══════════════════════════════════════════════════╡
    │ [[1, 2], [3]]                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[4, 5, 6]]                                      │
    └──────────────────────────────────────────────────┘
    ");

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌──────────────────────────────────────────────────┐
    │ col                                              │
    │ ---                                              │
    │ type: nullable List[nullable List[nullable i32]] │
    ╞══════════════════════════════════════════════════╡
    │ [[1, 2]]                                         │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[3]]                                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [[4, 5, 6]]                                      │
    └──────────────────────────────────────────────────┘
    ");
}

#[test]
fn test_explode_empty_input() {
    // Test exploding an empty list
    let input = ListArray::from_iter_primitive::<arrow::datatypes::Int32Type, _, _>(Vec::<
        Option<Vec<Option<i32>>>,
    >::new());

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    └───────────────────────────────────┘
    ");
}

#[test]
fn test_explode_with_skips_in_offset_buffer() {
    let values = Int32Array::from(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let offsets = OffsetBuffer::new(vec![0, 2, 7, 10].into());
    let validity = arrow::buffer::NullBuffer::from(vec![true, false, true]);
    let field = Arc::new(Field::new_list_field(DataType::Int32, true));

    let input = ListArray::new(field, offsets, Arc::new(values), Some(validity));

    insta::assert_snapshot!(format!("{}", DisplayRB(input.clone())), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [0, 1]                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [7, 8, 9]                         │
    └───────────────────────────────────┘
    ");

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌───────────────────────────────────┐
    │ col                               │
    │ ---                               │
    │ type: nullable List[nullable i32] │
    ╞═══════════════════════════════════╡
    │ [0]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [1]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null                              │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [7]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [8]                               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [9]                               │
    └───────────────────────────────────┘
    ");
}
