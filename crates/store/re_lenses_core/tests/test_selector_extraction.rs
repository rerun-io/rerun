mod util;

use arrow::datatypes::{DataType, Field, Fields};
use re_lenses_core::Selector;

use crate::util::fixtures;

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

    let result = re_lenses_core::extract_nested_fields(&datatype, |dt| {
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

    let result = re_lenses_core::extract_nested_fields(&datatype, |dt| {
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
    let result = re_lenses_core::extract_nested_fields(&array.value_type(), |dt| {
        matches!(dt, DataType::Float64)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .location.x (Float64)
    .location.y (Float64)
    ");

    let array = fixtures::nested_list_struct_column();
    let result = re_lenses_core::extract_nested_fields(&array.value_type(), |dt| {
        matches!(dt, DataType::Float64)
    })
    .expect("Should find nested fields");

    insta::assert_snapshot!(formatted(result), @"
    .poses[].x (Float64)
    .poses[].y (Float64)
    ");
}
