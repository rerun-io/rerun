use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanBufferBuilder, FixedSizeListArray, Float32Array, Float32Builder,
    Float64Array, Float64Builder, ListArray, StructArray,
};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

// ## Observations
//
// * The number of rows is identical, so transforming the contents is an affine mapping.

// Base trait for all arrow optics - provides a read-only view/projection
trait ArrowOptic: Clone {
    type Source: Array + Clone;
    type Target: Array + Clone;

    /// Preview/project from source to target (read-only, may fail)
    fn preview(&self, source: &Self::Source) -> Result<Self::Target, ArrowError>;
}

// Prism: Partial, reversible projection
// Represents operations that may fail but are conceptually reversible
// (e.g., field extraction - can extract field and inject it back into struct)
trait ArrowPrism: ArrowOptic {
    /// Review/inject target back into source structure
    fn review(&self, target: Self::Target) -> Result<Self::Source, ArrowError>;
}

#[derive(Clone)]
struct Compose<O1, O2> {
    first: O1,
    second: O2,
}

impl<O1, O2, M> ArrowOptic for Compose<O1, O2>
where
    O1: ArrowOptic<Target = M>,
    O2: ArrowOptic<Source = M>,
    M: Array,
{
    type Source = O1::Source;
    type Target = O2::Target;

    fn preview(&self, source: &Self::Source) -> Result<Self::Target, ArrowError> {
        let mid = self.first.preview(source)?;
        self.second.preview(&mid)
    }
}

// Extension trait for ergonomic composition
trait OpticExt: ArrowOptic {
    fn then<O2>(self, next: O2) -> Compose<Self, O2>
    where
        Self: Sized,
        O2: ArrowOptic<Source = Self::Target>,
    {
        Compose {
            first: self,
            second: next,
        }
    }
}

impl<T: ArrowOptic> OpticExt for T {}

// Prism: Extract field from StructArray
// Partial because field may not exist; reversible because we can inject field back
#[derive(Clone)]
struct StructFieldPrism {
    field_name: String,
}

impl ArrowOptic for StructFieldPrism {
    type Source = StructArray;
    type Target = ArrayRef;

    fn preview(&self, source: &StructArray) -> Result<ArrayRef, ArrowError> {
        source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} not found",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }
}

impl ArrowPrism for StructFieldPrism {
    fn review(&self, target: Self::Target) -> Result<Self::Source, ArrowError> {
        // Create a minimal struct with just this field
        let field = Field::new(&self.field_name, target.data_type().clone(), true);
        Ok(StructArray::new(
            vec![Arc::new(field)].into(),
            vec![target],
            None,
        ))
    }
}

// Traversal: Transform each element in ListArray using an inner optic
// This is a traversal pattern - applies inner optic to all list elements
#[derive(Clone)]
struct ListTraversal<O> {
    inner_optic: O,
}

impl<O> ArrowOptic for ListTraversal<O>
where
    O: ArrowOptic,
    O::Source: Array + 'static,
    O::Target: Array + 'static,
{
    type Source = ListArray;
    type Target = ListArray;

    fn preview(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<O::Source>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError("Type mismatch in ListTraversal".into())
        })?;

        let transformed = self.inner_optic.preview(downcast)?;
        let new_field = Arc::new(Field::new("item", transformed.data_type().clone(), true));

        let (_, offsets, _, nulls) = source.clone().into_parts();
        Ok(ListArray::new(
            new_field,
            offsets,
            Arc::new(transformed),
            nulls,
        ))
    }
}

// Getter: Convert struct {x, y} to FixedSizeListArray[2]
// This is a getter (read-only) - loses field names, so not reversible
#[derive(Clone)]
struct PointStructToFixedListOptic;

impl ArrowOptic for PointStructToFixedListOptic {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn preview(&self, source: &StructArray) -> Result<FixedSizeListArray, ArrowError> {
        let x_array = source
            .column_by_name("x")
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("Missing x field".into())
            })?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("x must be Float64".into())
            })?;

        let y_array = source
            .column_by_name("y")
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("Missing y field".into())
            })?
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError("y must be Float64".into())
            })?;

        let len = source.len();
        let mut builder = Float64Builder::with_capacity(len * 2);
        let mut null_buffer = BooleanBufferBuilder::new(len);

        for i in 0..len {
            if source.is_null(i) {
                builder.append_null();
                builder.append_null();
                null_buffer.append(false);
            } else {
                if x_array.is_null(i) || y_array.is_null(i) {
                    // Still append placeholder values, but mark the list entry as null
                    builder.append_null();
                    builder.append_null();
                    null_buffer.append(false);
                } else {
                    builder.append_value(x_array.value(i));
                    builder.append_value(y_array.value(i));
                    null_buffer.append(true);
                }
            }
        }

        let values = builder.finish();
        let field = Arc::new(Field::new_list_field(DataType::Float64, true));

        let null_buffer = Some(arrow::buffer::NullBuffer::new(null_buffer.finish()));

        Ok(FixedSizeListArray::new(
            field,
            2,
            Arc::new(values),
            null_buffer,
        ))
    }
}


// Prism: Unwrap single-element ListArray and extract field from the struct
// Partial because field may not exist; conceptually reversible
#[derive(Clone)]
struct UnwrapSingleStructFieldPrism {
    field_name: String,
}

impl ArrowOptic for UnwrapSingleStructFieldPrism {
    type Source = ListArray;
    type Target = ListArray;

    fn preview(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        // Get the struct array from the list values
        let struct_array = source
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Expected ListArray of StructArray".into(),
                )
            })?;

        // Extract the field
        let field_array = struct_array
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} not found",
                    self.field_name
                ))
            })?;

        // Downcast to ListArray
        field_array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field {} is not a ListArray",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }
}

impl ArrowPrism for UnwrapSingleStructFieldPrism {
    fn review(&self, target: Self::Target) -> Result<Self::Source, ArrowError> {
        // Create a struct with just this field
        let field = Field::new(&self.field_name, target.data_type().clone(), true);
        let struct_array = StructArray::new(
            vec![Arc::new(field)].into(),
            vec![Arc::new(target) as ArrayRef],
            None,
        );

        // Wrap in a list with one element per row
        let list_field = Arc::new(Field::new("item", struct_array.data_type().clone(), true));
        let mut offsets = Vec::with_capacity(struct_array.len() + 1);
        for i in 0..=struct_array.len() {
            offsets.push(i as i32);
        }
        let offsets = arrow::buffer::OffsetBuffer::new(offsets.into());

        Ok(ListArray::new(
            list_field,
            offsets,
            Arc::new(struct_array),
            None,
        ))
    }
}

// Getter: Transform Float64Array values with a function
// This is a getter - potentially non-invertible depending on the function
#[derive(Clone)]
struct Float64MapOptic<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    f: F,
}

impl<F> ArrowOptic for Float64MapOptic<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    type Source = Float64Array;
    type Target = Float64Array;

    fn preview(&self, source: &Float64Array) -> Result<Float64Array, ArrowError> {
        let mut builder = Float64Builder::with_capacity(source.len());
        for i in 0..source.len() {
            if source.is_null(i) {
                builder.append_null();
            } else {
                builder.append_value((self.f)(source.value(i)));
            }
        }
        Ok(builder.finish())
    }
}

// Getter: Convert Float64Array to Float32Array
// Lossy conversion - loses precision, so not reversible
#[derive(Clone)]
struct Float64ToFloat32Optic;

impl ArrowOptic for Float64ToFloat32Optic {
    type Source = Float64Array;
    type Target = Float32Array;

    fn preview(&self, source: &Float64Array) -> Result<Float32Array, ArrowError> {
        let mut builder = Float32Builder::with_capacity(source.len());
        for i in 0..source.len() {
            if source.is_null(i) {
                builder.append_null();
            } else {
                builder.append_value(source.value(i) as f32);
            }
        }
        Ok(builder.finish())
    }
}

// Traversal: Transform FixedSizeListArray values using inner optic
// Applies inner optic to the flattened values array
#[derive(Clone)]
struct FixedSizeListTraversal<O> {
    inner_optic: O,
}

impl<O> ArrowOptic for FixedSizeListTraversal<O>
where
    O: ArrowOptic,
    O::Source: Array + 'static,
    O::Target: Array + 'static,
{
    type Source = FixedSizeListArray;
    type Target = FixedSizeListArray;

    fn preview(&self, source: &FixedSizeListArray) -> Result<FixedSizeListArray, ArrowError> {
        let values = source
            .values()
            .as_any()
            .downcast_ref::<O::Source>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Type mismatch in FixedSizeListTraversal".into(),
                )
            })?;

        let transformed = self.inner_optic.preview(values)?;
        let field = Arc::new(Field::new("item", transformed.data_type().clone(), true));
        let size = source.value_length();
        let nulls = source.nulls().cloned();

        Ok(FixedSizeListArray::new(
            field,
            size,
            Arc::new(transformed),
            nulls,
        ))
    }
}

// Note: ListMapLens was redundant with ListTraversal - removing this duplicate

#[cfg(test)]
mod test {
    use super::*;

    use arrow::{
        array::{
            ArrayRef, Float64Builder, ListArray, ListBuilder, RecordBatch, RecordBatchOptions,
            StructBuilder,
        },
        datatypes::{DataType, Field, Fields, Schema},
    };

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
            Self(wrap_in_record_batch(Arc::new(array)))
        }
    }

    impl std::fmt::Display for DisplayRB {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", re_format_arrow::format_record_batch(&self.0))
        }
    }

    fn create_nasty_component_column() -> ListArray {
        let inner_struct_fields = Fields::from(vec![
            Field::new("x", DataType::Float64, true),
            Field::new("y", DataType::Float64, true),
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
            .append_value(7.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_null();
        inner.append(true);
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(7.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(7.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 2:
        let struct_val = column_builder.values();
        let list = struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap();
        let inner = list.values();
        inner
            .field_builder::<Float64Builder>(0)
            .unwrap()
            .append_value(17.0);
        inner
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(17.0);
        inner.append(true);
        list.append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 3:
        let struct_val = column_builder.values();
        struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap()
            .append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 3:
        column_builder.append(false);

        column_builder.finish()
    }

    #[test]
    fn simple() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = UnwrapSingleStructFieldPrism {
            field_name: "poses".into(),
        }
        .then(ListTraversal {
            inner_optic: PointStructToFixedListOptic,
        });

        let result: ListArray = pipeline.preview(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapSingleStructFieldPrism {
            field_name: "poses".into(),
        }
        .then(ListTraversal {
            inner_optic: PointStructToFixedListOptic,
        })
        .then(ListTraversal {
            inner_optic: FixedSizeListTraversal {
                inner_optic: Float64MapOptic { f: |x| x + 1.0 },
            },
        });

        let result = pipeline.preview(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapSingleStructFieldPrism {
            field_name: "poses".into(),
        }
        .then(ListTraversal {
            inner_optic: PointStructToFixedListOptic,
        })
        .then(ListTraversal {
            inner_optic: FixedSizeListTraversal {
                inner_optic: Float64ToFloat32Optic,
            },
        });

        let result = pipeline.preview(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }
}
