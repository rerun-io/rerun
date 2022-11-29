use arrow2::{
    array::{Array, ListArray},
    buffer::Buffer,
    chunk::Chunk,
};
use arrow2_convert::serialize::TryIntoArrow;
use polars::export::arrow::datatypes::Field;

use super::*;

/// Wrap `field_array` in a single-element `ListArray`
fn wrap_in_listarray(field_array: Box<dyn Array>) -> ListArray<i32> {
    ListArray::<i32>::from_data(
        ListArray::<i32>::default_datatype(field_array.data_type().clone()), // datatype
        Buffer::from(vec![0, field_array.len() as i32]),                     // offsets
        field_array,                                                         // values
        None,                                                                // validity
    )
}

/// Create `len` dummy rectangles
fn build_some_rects(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| field_types::Rect2D {
            x: i as f32,
            y: i as f32,
            w: (i / 2) as f32,
            h: (i / 2) as f32,
        })
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Create `len` dummy colors

fn build_some_colors(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| i as field_types::ColorRGBA)
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Create `len` dummy labels
fn build_some_labels(len: usize) -> Box<dyn Array> {
    let v = (0..len)
        .into_iter()
        .map(|i| format!("label{i}"))
        .collect::<Vec<_>>();
    v.try_into_arrow().unwrap()
}

/// Build a sample row of Rect data
fn build_test_rect_chunk() -> (Chunk<Box<dyn Array>>, ArrowSchema) {
    let time = arrow2::array::UInt32Array::from_slice([1234]).boxed();
    let rect = wrap_in_listarray(build_some_rects(5)).boxed();
    let color = wrap_in_listarray(build_some_colors(5)).boxed();
    let label = wrap_in_listarray(build_some_labels(1)).boxed();

    let schema = vec![
        Field::new("log_time", time.data_type().clone(), false),
        Field::new("rect", rect.data_type().clone(), true),
        Field::new("color", color.data_type().clone(), true),
        Field::new("label", label.data_type().clone(), true),
    ]
    .into();
    let chunk = Chunk::new(vec![time, rect, color, label]);
    (chunk, schema)
}

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
