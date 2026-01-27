mod util;

use arrow::array::ListArray;
use arrow::datatypes::Int32Type;
use re_arrow_combinators::Transform as _;
use re_arrow_combinators::reshape::GetIndexList;
use util::DisplayRB;

#[test]
fn get_index_list_primitives() {
    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2), Some(3)]),
        Some(vec![Some(4), Some(5)]),
        Some(vec![Some(6)]),
    ]);

    let result = GetIndexList::new(0).transform(&input).unwrap();
    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌────────────────────┐
    │ col                │
    │ ---                │
    │ type: nullable i32 │
    ╞════════════════════╡
    │ 1                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 4                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 6                  │
    └────────────────────┘
    ");

    let result = GetIndexList::new(1).transform(&input).unwrap();
    insta::assert_snapshot!( format!("{}", DisplayRB(result)), @"
    ┌────────────────────┐
    │ col                │
    │ ---                │
    │ type: nullable i32 │
    ╞════════════════════╡
    │ 2                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ 5                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    └────────────────────┘
    ");
}

#[test]
fn get_index_list_with_nulls() {
    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        None,
        Some(vec![Some(3), None, Some(5)]),
        Some(vec![]),
    ]);

    let result = GetIndexList::new(1).transform(&input).unwrap();

    insta::assert_snapshot!( format!("{}", DisplayRB(result)), @"
    ┌────────────────────┐
    │ col                │
    │ ---                │
    │ type: nullable i32 │
    ╞════════════════════╡
    │ 2                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    └────────────────────┘
    ");
}

#[test]
fn get_index_list_out_of_bounds() {
    let input = ListArray::from_iter_primitive::<Int32Type, _, _>(vec![
        Some(vec![Some(1), Some(2)]),
        Some(vec![Some(3)]),
        Some(vec![]),
    ]);

    let result = GetIndexList::new(5).transform(&input).unwrap();

    insta::assert_snapshot!(format!("{}", DisplayRB(result)), @"
    ┌────────────────────┐
    │ col                │
    │ ---                │
    │ type: nullable i32 │
    ╞════════════════════╡
    │ null               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ null               │
    └────────────────────┘
    ");
}
