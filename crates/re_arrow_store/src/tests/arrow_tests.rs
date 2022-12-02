use super::*;

#[test]
fn test_rect_chunk() {
    let (chunk, schema) = build_test_rect_chunk();
    let x = polars::prelude::DataFrame::try_from((chunk, schema.fields.as_slice()));
    println!("{x:?}");
}

//--- Old tests --

#[test]
fn test_time_query() {
    let mut df1: DataFrame = df!(
        "time" => &[1, 3, 2],
        "numeric" => &[None, None, Some(3)],
        "object" => &[None, Some("b"), None],
        "dat" => &[Some(99), None, Some(66)],
    )
    .unwrap();

    let _df_sorted = df1.sort_in_place(["time"], false).unwrap();
}

#[test]
fn test_append_unified() {
    let mut df1 = df!(
        "colA" => [1, 2, 3],
        "colB" => ["one", "two", "three"],
    )
    .unwrap();

    let df2 = df!(
        "colA" => [4, 5, 6],
        "colC" => [Some(0.0), Some(0.1), None],
    )
    .unwrap();

    append_unified(&mut df1, &df2).unwrap();
}
