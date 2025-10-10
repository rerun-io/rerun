//! Provides commonly used transformations of Arrow arrays.
//!
//! These operations should not be exposed publicly, but instead be wrapped by the [`super::ast::Op`] abstraction.

// TODO(grtlr): Eventually we will want to make the types in here compatible with Datafusion UDFs.

use std::sync::Arc;

use arrow::datatypes::Fields;
use re_chunk::{
    ArrowArray as _,
    external::arrow::{
        array::{ListArray, StructArray},
        compute,
        datatypes::{DataType, Field},
    },
};

use super::Error;

/// Extracts a specific field from a struct component within a `ListArray`.
#[derive(Debug)]
pub struct AccessField {
    pub(super) field_name: String,
}

impl AccessField {
    pub fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        let (field, offsets, values, nulls) = list_array.into_parts();
        let struct_array = values
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                actual: field.data_type().clone(),
                // TODO(grtlr): The struct should not actually be empty, but rather
                // contain the field that we're looking for. But we don't know it's
                // type here. When we implement schemas for ops, we probably need
                // to create our own wrapper types (similar to Datafusion).
                expected: DataType::Struct(Fields::empty()),
            })?;

        let column = struct_array
            .column_by_name(&self.field_name)
            .ok_or_else(|| Error::MissingField {
                expected: self.field_name.clone(),
                found: struct_array
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect(),
            })?;

        Ok(ListArray::new(
            Arc::new(Field::new_list_field(column.data_type().clone(), true)),
            offsets,
            column.clone(),
            nulls,
        ))
    }
}

/// Casts the `value_type` (inner array) of a `ListArray` to a different data type.
#[derive(Debug)]
pub struct Cast {
    pub(super) to_inner_type: DataType,
}

impl Cast {
    pub fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        let (_field, offsets, ref array, nulls) = list_array.into_parts();
        let res = compute::cast(array, &self.to_inner_type)?;
        Ok(ListArray::new(
            Arc::new(Field::new_list_field(res.data_type().clone(), true)),
            offsets,
            res,
            nulls,
        ))
    }
}

/// Flattens a `ListArray` of `ListArray`
#[derive(Debug)]
pub struct Flatten;

impl Flatten {
    pub fn call(&self, list_array: ListArray) -> Result<ListArray, Error> {
        let (field, outer_offsets, values, outer_nulls) = list_array.into_parts();

        let inner_list_array =
            values
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| Error::TypeMismatch {
                    actual: field.data_type().clone(),
                    expected: DataType::List(Arc::new(Field::new("item", DataType::Null, true))),
                })?;

        // Check that each row only contains a single `ListArray`.
        if inner_list_array.len() != outer_offsets.len() - 1 {
            return Err(Error::UnsupportedOperation(
                "Flatten only supports rows with a single list array".to_owned(),
            ));
        }

        let (inner_field, inner_offsets, inner_values, _inner_nulls) =
            inner_list_array.clone().into_parts();

        // Build new offsets by mapping outer offsets through inner offsets
        let new_offsets: Vec<i32> = outer_offsets
            .iter()
            .map(|&outer_offset| inner_offsets[outer_offset as usize])
            .collect();

        Ok(ListArray::new(
            inner_field,
            arrow::buffer::OffsetBuffer::new(new_offsets.into()),
            inner_values,
            outer_nulls,
        ))
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use arrow::{
        array::{
            ArrayRef, Float64Builder, ListArray, ListBuilder, RecordBatch, RecordBatchOptions,
            StructBuilder,
        },
        datatypes::{DataType, Field, Fields, Schema},
    };

    use crate::lenses::op::{AccessField, Flatten};

    /// Helper function to wrap an [`ArrayRef`] into a [`RecordBatch`] for easier printing.
    fn wrap_in_record_batch(array: ArrayRef) -> RecordBatch {
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("col", array.data_type().clone(), true)],
            Default::default(),
        ));
        RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default())
            .unwrap()
    }

    struct DisplayRB(RecordBatch);

    impl From<ListArray> for DisplayRB {
        fn from(array: ListArray) -> Self {
            DisplayRB(wrap_in_record_batch(Arc::new(array)))
        }
    }

    impl std::fmt::Display for DisplayRB {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", re_format_arrow::format_record_batch(&self.0))
        }
    }

    fn create_nasty_component_column() -> ListArray {
        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, false),
            Field::new("y", DataType::Float64, false),
        ]);

        // Middle struct schema: {poses: List<Struct<x: Float32>>}
        let middle_struct_fields = Fields::from(vec![Field::new(
            "poses",
            DataType::List(Arc::new(Field::new(
                "item",
                DataType::Struct(inner_struct_fields.clone()),
                false,
            ))),
            false,
        )]);

        // Construct nested builders
        let inner_struct_builder = StructBuilder::new(
            inner_struct_fields.clone(),
            vec![
                Box::new(Float64Builder::new()),
                Box::new(Float64Builder::new()),
            ],
        );

        let list_builder = ListBuilder::new(inner_struct_builder).with_field(Arc::new(Field::new(
            "item",
            DataType::Struct(inner_struct_fields),
            false,
        )));

        let struct_builder = StructBuilder::new(middle_struct_fields, vec![Box::new(list_builder)]);

        let mut column_builder = ListBuilder::new(struct_builder);

        // Row 0:
        let struct_val = column_builder.values();
        let list = struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap();
        let inner = list.values();
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(0.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(0.0);
        inner.append(true);
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(42.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(42.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 1:
        let struct_val = column_builder.values();
        let list = struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap();
        let inner = list.values();
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(0.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(0.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 2:
        let struct_val = column_builder.values();
        struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap()
            .append(true);
        struct_val.append(true);
        column_builder.append(true);

        column_builder.finish()
    }

    #[test]
    fn field_and_flatten() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let res = AccessField {
            field_name: "poses".into(),
        }
        .call(array)
        .unwrap();

        insta::assert_snapshot!("field", DisplayRB::from(res.clone()));

        let res = Flatten.call(res).unwrap();

        insta::assert_snapshot!("field_and_flatten", DisplayRB::from(res));
    }
}
