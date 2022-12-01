mod arrow_tests;

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

//TODO(john) move these build test functions into an example

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
