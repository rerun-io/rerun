use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanBufferBuilder, FixedSizeListArray, Float32Array, Float32Builder,
    Float64Array, Float64Builder, ListArray, StructArray,
};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

// ## Arrow Transformations
//
// This module provides composable transformations for Arrow arrays.
// All operations preserve row count (affine transformations).

/// A transformation that projects from one Arrow array type to another.
///
/// This is a read-only projection that may fail (e.g., missing field).
/// Mathematical property: Affine transformation (0 or 1 output per input).
trait Projection: Clone {
    type Source: Array + Clone;
    type Target: Array + Clone;

    /// Project from source array to target array.
    fn project(&self, source: &Self::Source) -> Result<Self::Target, ArrowError>;
}

/// Applies a transformation to multiple elements within a container.
///
/// Example: Transform each element in a list using an inner projection.
/// Mathematical property: Non-affine transformation (0 to N outputs per input).
trait ElementwiseTransform: Clone {
    type Container: Array + Clone;
    type Element: Array + Clone;

    /// Apply a projection to all elements in the container.
    fn transform<P>(&self, source: &Self::Container, projection: &P) -> Result<Self::Container, ArrowError>
    where
        P: Projection<Source = Self::Element>,
        P::Target: Array + 'static;
}

/// Composes two projections into a single projection.
#[derive(Clone)]
struct Compose<P1, P2> {
    first: P1,
    second: P2,
}

impl<P1, P2, M> Projection for Compose<P1, P2>
where
    P1: Projection<Target = M>,
    P2: Projection<Source = M>,
    M: Array,
{
    type Source = P1::Source;
    type Target = P2::Target;

    fn project(&self, source: &Self::Source) -> Result<Self::Target, ArrowError> {
        let mid = self.first.project(source)?;
        self.second.project(&mid)
    }
}

/// Extension trait for composing projections.
trait ProjectionExt: Projection {
    /// Chain this projection with another projection.
    fn then<P2>(self, next: P2) -> Compose<Self, P2>
    where
        Self: Sized,
        P2: Projection<Source = Self::Target>,
    {
        Compose {
            first: self,
            second: next,
        }
    }

    /// Chain this projection with an elementwise transformation.
    fn then_each<T, P2>(self, transform: T, inner: P2) -> Compose<Self, ApplyToElements<T, P2>>
    where
        Self: Sized,
        T: ElementwiseTransform<Container = Self::Target>,
        P2: Projection<Source = T::Element>,
        P2::Target: Array + 'static,
    {
        self.then(ApplyToElements { transform, projection: inner })
    }
}

impl<T: Projection> ProjectionExt for T {}

/// Applies a projection to each element in a container.
#[derive(Clone)]
struct ApplyToElements<T, P> {
    transform: T,
    projection: P,
}

impl<T, P> Projection for ApplyToElements<T, P>
where
    T: ElementwiseTransform,
    P: Projection<Source = T::Element>,
    P::Target: Array + 'static,
{
    type Source = T::Container;
    type Target = T::Container;

    fn project(&self, source: &Self::Source) -> Result<Self::Target, ArrowError> {
        self.transform.transform(source, &self.projection)
    }
}

/// Extracts a field from a struct array.
///
/// Fails if the field doesn't exist.
#[derive(Clone)]
struct ExtractStructField {
    field_name: String,
}

impl Projection for ExtractStructField {
    type Source = StructArray;
    type Target = ArrayRef;

    fn project(&self, source: &StructArray) -> Result<ArrayRef, ArrowError> {
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

/// Transforms the elements within a list array.
///
/// Applies a projection to the flattened values array inside the list.
/// Generic over element type for type safety.
#[derive(Clone)]
struct TransformListElements<E> {
    _phantom: std::marker::PhantomData<E>,
}

impl<E> TransformListElements<E> {
    fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<E: Array + Clone + 'static> ElementwiseTransform for TransformListElements<E> {
    type Container = ListArray;
    type Element = E;

    fn transform<P>(&self, source: &Self::Container, projection: &P) -> Result<Self::Container, ArrowError>
    where
        P: Projection<Source = Self::Element>,
        P::Target: Array + 'static,
    {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<E>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError(format!(
                "Type mismatch: expected {}",
                std::any::type_name::<E>()
            ))
        })?;

        let transformed = projection.project(downcast)?;
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

/// Converts a struct with {x, y} fields to a fixed-size list array of length 2.
///
/// Loses field name information during conversion.
#[derive(Clone)]
struct ConvertPointStructToFixedList;

impl Projection for ConvertPointStructToFixedList {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn project(&self, source: &StructArray) -> Result<FixedSizeListArray, ArrowError> {
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


/// Unwraps a single-element struct array within a list and extracts a field.
///
/// Input: ListArray[StructArray] -> Output: the field from the struct.
/// Fails if the field doesn't exist or isn't a ListArray.
#[derive(Clone)]
struct UnwrapStructAndExtractField {
    field_name: String,
}

impl Projection for UnwrapStructAndExtractField {
    type Source = ListArray;
    type Target = ListArray;

    fn project(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
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

/// Applies a function to each element in a Float64Array.
#[derive(Clone)]
struct MapFloat64<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    f: F,
}

impl<F> Projection for MapFloat64<F>
where
    F: Fn(f64) -> f64 + Clone,
{
    type Source = Float64Array;
    type Target = Float64Array;

    fn project(&self, source: &Float64Array) -> Result<Float64Array, ArrowError> {
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

/// Converts Float64Array to Float32Array.
///
/// Lossy conversion - reduces precision from 64-bit to 32-bit floats.
#[derive(Clone)]
struct ConvertFloat64ToFloat32;

impl Projection for ConvertFloat64ToFloat32 {
    type Source = Float64Array;
    type Target = Float32Array;

    fn project(&self, source: &Float64Array) -> Result<Float32Array, ArrowError> {
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

/// Transforms the elements within a fixed-size list array.
///
/// Applies a projection to the flattened values array inside the list.
/// Generic over element type for type safety.
#[derive(Clone)]
struct TransformFixedSizeListElements<E> {
    _phantom: std::marker::PhantomData<E>,
}

impl<E> TransformFixedSizeListElements<E> {
    fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<E: Array + Clone + 'static> ElementwiseTransform for TransformFixedSizeListElements<E> {
    type Container = FixedSizeListArray;
    type Element = E;

    fn transform<P>(&self, source: &Self::Container, projection: &P) -> Result<Self::Container, ArrowError>
    where
        P: Projection<Source = Self::Element>,
        P::Target: Array + 'static,
    {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<E>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError(format!(
                "Type mismatch: expected {}",
                std::any::type_name::<E>()
            ))
        })?;

        let transformed = projection.project(downcast)?;
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

        let pipeline = UnwrapStructAndExtractField {
            field_name: "poses".into(),
        }
        .then_each(TransformListElements::new(), ConvertPointStructToFixedList);

        let result: ListArray = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapStructAndExtractField {
            field_name: "poses".into(),
        }
        .then_each(TransformListElements::new(), ConvertPointStructToFixedList)
        .then_each(TransformListElements::new(), ApplyToElements {
            transform: TransformFixedSizeListElements::new(),
            projection: MapFloat64 { f: |x| x + 1.0 },
        });

        let result = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapStructAndExtractField {
            field_name: "poses".into(),
        }
        .then_each(TransformListElements::new(), ConvertPointStructToFixedList)
        .then_each(TransformListElements::new(), ApplyToElements {
            transform: TransformFixedSizeListElements::new(),
            projection: ConvertFloat64ToFloat32,
        });

        let result = pipeline.project(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }
}
