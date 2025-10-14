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
// Transformations are composable operations that convert one array type to another,
// preserving structural properties like row counts and null handling.

/// A transformation that converts one Arrow array type to another.
///
/// Transformations are read-only operations that may fail (e.g., missing field, type mismatch).
/// They can be composed using the `then` method to create complex transformation pipelines.
///
/// # Examples
///
/// ```ignore
/// let transform = GetField::new("x").then(ToFloat32::new());
/// let result = transform.transform(&my_array)?;
/// ```
pub trait Transform {
    type Source: Array;
    type Target: Array;

    /// Apply the transformation to the source array.
    fn transform(&self, source: &Self::Source) -> Result<Self::Target, ArrowError>;
}

/// Composes two transformations into a single transformation.
///
/// This is the result of calling `.then()` on a transformation.
#[derive(Clone)]
pub struct Compose<T1, T2> {
    first: T1,
    second: T2,
}

impl<T1, T2, M> Transform for Compose<T1, T2>
where
    T1: Transform<Target = M>,
    T2: Transform<Source = M>,
    M: Array,
{
    type Source = T1::Source;
    type Target = T2::Target;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, ArrowError> {
        let mid = self.first.transform(source)?;
        self.second.transform(&mid)
    }
}

/// Extension trait for composing transformations.
pub trait TransformExt: Transform {
    /// Chain this transformation with another transformation.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let pipeline = GetField::new("data")
    ///     .then(MapList::new(ToFloat32::new()));
    /// ```
    fn then<T2>(self, next: T2) -> Compose<Self, T2>
    where
        Self: Sized,
        T2: Transform<Source = Self::Target>,
    {
        Compose {
            first: self,
            second: next,
        }
    }
}

impl<T: Transform> TransformExt for T {}

/// Extracts a field from a struct array.
///
/// Returns the field's array if it exists, otherwise returns an error.
///
/// # Examples
///
/// ```ignore
/// let get_x = GetField::new("x");
/// let x_values = get_x.transform(&struct_array)?;
/// ```
#[derive(Clone)]
pub struct GetField {
    field_name: String,
}

impl GetField {
    /// Create a new field extractor for the given field name.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
        }
    }
}

impl Transform for GetField {
    type Source = StructArray;
    type Target = ArrayRef;

    fn transform(&self, source: &StructArray) -> Result<ArrayRef, ArrowError> {
        source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field '{}' not found in struct",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }
}

/// Maps a transformation over the elements within a list array.
///
/// Applies the inner transformation to the flattened values array while preserving
/// the list structure (offsets and null bitmap).
///
/// # Examples
///
/// ```ignore
/// // Convert all floats in a list to f32
/// let transform = MapList::new(ToFloat32::new());
/// let result = transform.transform(&list_of_f64)?;
/// ```
#[derive(Clone)]
pub struct MapList<T> {
    transform: T,
}

impl<T> MapList<T> {
    /// Create a new list mapper that applies the given transformation to list elements.
    pub fn new(transform: T) -> Self {
        Self { transform }
    }
}

impl<T, S, U> Transform for MapList<T>
where
    T: Transform<Source = S, Target = U>,
    S: Array + 'static,
    U: Array + 'static,
{
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<S>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError(format!(
                "Type mismatch in list values: expected {}, got {:?}",
                std::any::type_name::<S>(),
                values.data_type()
            ))
        })?;

        let transformed = self.transform.transform(downcast)?;
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

/// Maps a transformation over the elements within a fixed-size list array.
///
/// Applies the inner transformation to the flattened values array while preserving
/// the fixed-size list structure (element count and null bitmap).
///
/// # Examples
///
/// ```ignore
/// // Add 1.0 to each element in fixed-size lists
/// let transform = MapFixedSizeList::new(MapFloat64::new(|x| x + 1.0));
/// let result = transform.transform(&fixed_list_array)?;
/// ```
#[derive(Clone)]
pub struct MapFixedSizeList<T> {
    transform: T,
}

impl<T> MapFixedSizeList<T> {
    /// Create a new fixed-size list mapper that applies the given transformation to list elements.
    pub fn new(transform: T) -> Self {
        Self { transform }
    }
}

impl<T, S, U> Transform for MapFixedSizeList<T>
where
    T: Transform<Source = S, Target = U>,
    S: Array + 'static,
    U: Array + 'static,
{
    type Source = FixedSizeListArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &FixedSizeListArray) -> Result<FixedSizeListArray, ArrowError> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<S>().ok_or_else(|| {
            arrow::error::ArrowError::InvalidArgumentError(format!(
                "Type mismatch in fixed-size list values: expected {}, got {:?}",
                std::any::type_name::<S>(),
                values.data_type()
            ))
        })?;

        let transformed = self.transform.transform(downcast)?;
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

/// Converts a struct with {x, y} fields to a fixed-size list array of length 2.
///
/// This transformation extracts the x and y fields from each struct and packs them
/// into a fixed-size list of 2 elements. Null handling: if either x or y is null,
/// the entire list entry is marked as null.
///
/// # Examples
///
/// ```ignore
/// // Transform struct{x: f64, y: f64} to FixedSizeList[2] of f64
/// let transform = StructToPoint2D::new();
/// let points = transform.transform(&struct_array)?;
/// ```
#[derive(Clone)]
pub struct StructToPoint2D;

impl StructToPoint2D {
    /// Create a new struct-to-point-2D transformer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StructToPoint2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform for StructToPoint2D {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &StructArray) -> Result<FixedSizeListArray, ArrowError> {
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
            // Mark the list entry as null if the struct is null or either field is null
            let is_valid = !source.is_null(i) && !x_array.is_null(i) && !y_array.is_null(i);

            if is_valid {
                builder.append_value(x_array.value(i));
                builder.append_value(y_array.value(i));
                null_buffer.append(true);
            } else {
                // Append placeholder nulls for the list elements
                builder.append_null();
                builder.append_null();
                null_buffer.append(false);
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


/// Unwraps a struct array within a list and extracts a field from it.
///
/// This is useful for nested structures like `List<Struct<field: List<T>>>`,
/// where you want to extract the inner field directly.
///
/// # Examples
///
/// ```ignore
/// // Transform List<Struct<poses: List<Point>>> to List<Point>
/// let transform = UnwrapListStructField::new("poses");
/// let poses = transform.transform(&nested_list)?;
/// ```
#[derive(Clone)]
pub struct UnwrapListStructField {
    field_name: String,
}

impl UnwrapListStructField {
    /// Create a new transformer that unwraps a struct within a list and extracts the given field.
    pub fn new(field_name: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
        }
    }
}

impl Transform for UnwrapListStructField {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<ListArray, ArrowError> {
        // Get the struct array from the list values
        let struct_array = source
            .values()
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(
                    "Expected list values to be a StructArray".into(),
                )
            })?;

        // Extract the field
        let field_array = struct_array
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field '{}' not found in struct",
                    self.field_name
                ))
            })?;

        // Downcast to ListArray
        field_array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| {
                arrow::error::ArrowError::InvalidArgumentError(format!(
                    "Field '{}' is not a ListArray",
                    self.field_name
                ))
            })
            .map(Clone::clone)
    }
}

/// Maps a function over each element in a Float64Array.
///
/// Applies the given function to each non-null element, preserving null values.
///
/// # Examples
///
/// ```ignore
/// // Add 1.0 to each element
/// let transform = MapFloat64::new(|x| x + 1.0);
/// let result = transform.transform(&float_array)?;
/// ```
#[derive(Clone)]
pub struct MapFloat64<F>
where
    F: Fn(f64) -> f64,
{
    f: F,
}

impl<F> MapFloat64<F>
where
    F: Fn(f64) -> f64,
{
    /// Create a new mapper that applies the given function to each f64 element.
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F> Transform for MapFloat64<F>
where
    F: Fn(f64) -> f64,
{
    type Source = Float64Array;
    type Target = Float64Array;

    fn transform(&self, source: &Float64Array) -> Result<Float64Array, ArrowError> {
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
/// This is a lossy conversion that reduces precision from 64-bit to 32-bit floats.
/// Null values are preserved.
///
/// # Examples
///
/// ```ignore
/// let transform = ToFloat32::new();
/// let f32_array = transform.transform(&f64_array)?;
/// ```
#[derive(Clone)]
pub struct ToFloat32;

impl ToFloat32 {
    /// Create a new f64-to-f32 converter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ToFloat32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform for ToFloat32 {
    type Source = Float64Array;
    type Target = Float32Array;

    fn transform(&self, source: &Float64Array) -> Result<Float32Array, ArrowError> {
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

        let pipeline = UnwrapListStructField::new("poses")
            .then(MapList::new(StructToPoint2D::new()));

        let result: ListArray = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapListStructField::new("poses")
            .then(MapList::new(StructToPoint2D::new()))
            .then(MapList::new(MapFixedSizeList::new(MapFloat64::new(|x| x + 1.0))));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapListStructField::new("poses")
            .then(MapList::new(StructToPoint2D::new()))
            .then(MapList::new(MapFixedSizeList::new(ToFloat32::new())));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }
}
