mod util;

use std::sync::Arc;

use arrow::array::{Array as _, Int32Array, ListArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Int32Type};
use re_arrow_combinators::{Transform as _, reshape::Explode};
use util::DisplayRB;

#[test]
fn test_explode_primitives() {
    // Test exploding a List<Int32>
    // Input: [[1, 2, 3], [4, 5], [6]]
    // Output: [[1], [2], [3], [4], [5], [6]]

    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2), Some(3)]),
        Some(vec![Some(4), Some(5)]),
        Some(vec![Some(6)]),
    ]);
    println!("Input:\n{}", DisplayRB(input.clone()));

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!("primitives", format!("{}", DisplayRB(result)));
}

#[test]
fn test_explode_with_nulls_and_empty() {
    // Test exploding with null and empty arrays
    // Input: [[1, 2], null, [], [3]]
    // Output: [[1], [2], null, [], [3]]

    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        None,
        Some(vec![]),
        Some(vec![Some(3)]),
    ]);
    println!("Input:\n{}", DisplayRB(input.clone()));

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!("nulls_and_empty", format!("{}", DisplayRB(result)));
}

#[test]
fn test_explode_nested_lists() {
    // Test exploding a List<List<Int32>>
    // Input: [[[1, 2], [3]], [[4, 5, 6]]]
    // Output: [[1, 2], [3], [4, 5, 6]]

    // Manually build List<List<Int32>>
    let inner_values = Int32Array::from(vec![1, 2, 3, 4, 5, 6]);
    let inner_offsets = OffsetBuffer::new(vec![0, 2, 3, 6].into());
    let inner_field = Arc::new(Field::new("item", DataType::Int32, true));
    let inner_list = ListArray::new(inner_field, inner_offsets, Arc::new(inner_values), None);

    let outer_offsets = OffsetBuffer::new(vec![0, 2, 3].into());
    let outer_field = Arc::new(Field::new("item", inner_list.data_type().clone(), true));
    let input = ListArray::new(
        outer_field,
        outer_offsets,
        Arc::new(inner_list.clone()),
        None,
    );
    println!("Input:\n{}", DisplayRB(input.clone()));

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!("nested_lists", format!("{}", DisplayRB(result)));
}

#[test]
fn test_explode_empty_input() {
    // Test exploding an empty list
    let input = ListArray::from_iter_primitive::<arrow::datatypes::Int32Type, _, _>(Vec::<
        Option<Vec<Option<i32>>>,
    >::new());
    println!("Input:\n{}", DisplayRB(input.clone()));

    let explode = Explode;
    let result = explode.transform(&input).unwrap();

    insta::assert_snapshot!("empty_input", format!("{}", DisplayRB(result)));
}
