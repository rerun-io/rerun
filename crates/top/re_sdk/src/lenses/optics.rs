use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanBufferBuilder, FixedSizeListArray, Float32Array, Float64Array,
    Float64Builder, ListArray, PrimitiveArray, StructArray, ArrowPrimitiveType,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

// ## Arrow Transformations
//
// This module provides composable transformations for Arrow arrays.
// Transformations are composable operations that convert one array type to another,
// preserving structural properties like row counts and null handling.

/// Errors that can occur during array transformations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A required field was not found in a struct array.
    #[error("Field '{field_name}' not found. Available fields: [{}]", available_fields.join(", "))]
    FieldNotFound {
        field_name: String,
        available_fields: Vec<String>,
    },

    /// A field exists but has the wrong type.
    #[error("Field '{field_name}' has wrong type: expected {expected_type}, got {actual_type:?}")]
    FieldTypeMismatch {
        field_name: String,
        expected_type: String,
        actual_type: DataType,
    },

    /// Array type mismatch during transformation.
    #[error("Type mismatch in {context}: expected {expected}, got {actual:?}")]
    TypeMismatch {
        expected: String,
        actual: DataType,
        context: String,
    },

    /// A required field is missing from a struct.
    #[error("Struct is missing required field '{field_name}'. Available fields: [{}]", struct_fields.join(", "))]
    MissingStructField {
        field_name: String,
        struct_fields: Vec<String>,
    },

    /// List values have unexpected type.
    #[error("List contains unexpected value type: expected {expected}, got {actual:?}")]
    UnexpectedListValueType {
        expected: String,
        actual: DataType,
    },

    /// Fixed-size list values have unexpected type.
    #[error("Fixed-size list contains unexpected value type: expected {expected}, got {actual:?}")]
    UnexpectedFixedSizeListValueType {
        expected: String,
        actual: DataType,
    },

    /// Struct values in list have wrong type.
    #[error("Expected list to contain struct values, but got {actual:?}")]
    ExpectedStructInList {
        actual: DataType,
    },

    /// Fields have inconsistent types.
    #[error("Field '{field_name}' has type {actual_type:?}, but expected {expected_type:?} (inferred from field '{reference_field}')")]
    InconsistentFieldTypes {
        field_name: String,
        actual_type: DataType,
        reference_field: String,
        expected_type: DataType,
    },

    /// No field names provided to transformation.
    #[error("At least one field name is required")]
    NoFieldNames,

    /// Custom user-defined error for transformations implemented outside this module.
    ///
    /// This allows users to implement their own transformations with custom error messages.
    #[error("{0}")]
    Custom(String),

    /// Arrow library error.
    ///
    /// This is used to wrap errors from the underlying Arrow library operations.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

impl Error {
    /// Create a custom error with a user-defined message.
    ///
    /// This is useful when implementing custom transformations that need
    /// application-specific error messages.
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

/// A transformation that converts one Arrow array type to another.
///
/// Transformations are read-only operations that may fail (e.g., missing field, type mismatch).
/// They can be composed using the `then` method to create complex transformation pipelines.
pub trait Transform {
    type Source: Array;
    type Target: Array;

    /// Apply the transformation to the source array.
    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error>;
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

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error> {
        let mid = self.first.transform(source)?;
        self.second.transform(&mid)
    }
}

/// Extension trait for composing transformations.
pub trait TransformExt: Transform {
    /// Chain this transformation with another transformation.
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

    fn transform(&self, source: &StructArray) -> Result<ArrayRef, Error> {
        source
            .column_by_name(&self.field_name)
            .ok_or_else(|| {
                let available_fields = source
                    .fields()
                    .iter()
                    .map(|f| f.name().clone())
                    .collect();
                Error::FieldNotFound {
                    field_name: self.field_name.clone(),
                    available_fields,
                }
            })
            .map(Clone::clone)
    }
}

/// Maps a transformation over the elements within a list array.
///
/// Applies the inner transformation to the flattened values array while preserving
/// the list structure (offsets and null bitmap).
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

    fn transform(&self, source: &ListArray) -> Result<ListArray, Error> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<S>().ok_or_else(|| {
            Error::UnexpectedListValueType {
                expected: std::any::type_name::<S>().to_string(),
                actual: values.data_type().clone(),
            }
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

    fn transform(&self, source: &FixedSizeListArray) -> Result<FixedSizeListArray, Error> {
        let values = source.values();
        let downcast = values.as_any().downcast_ref::<S>().ok_or_else(|| {
            Error::UnexpectedFixedSizeListValueType {
                expected: std::any::type_name::<S>().to_string(),
                actual: values.data_type().clone(),
            }
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

/// Converts a struct to a fixed-size list array by extracting specified fields.
///
/// This transformation takes a list of field names and extracts them from each struct,
/// packing them into a fixed-size list. The size of the list equals the number of field names.
///
/// Null handling: Individual field values can be null (represented as null in the flattened array),
/// but the outer list entries are never null - missing fields result in null values in the list.
#[derive(Clone)]
pub struct StructToFixedList {
    field_names: Vec<String>,
}

impl StructToFixedList {
    /// Create a new struct-to-fixed-list transformer.
    ///
    /// The field names specify which fields to extract and in what order.
    /// The resulting fixed-size list will have length equal to `field_names.len()`.
    pub fn new(field_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            field_names: field_names.into_iter().map(|s| s.into()).collect(),
        }
    }
}

impl Transform for StructToFixedList {
    type Source = StructArray;
    type Target = FixedSizeListArray;

    fn transform(&self, source: &StructArray) -> Result<FixedSizeListArray, Error> {
        if self.field_names.is_empty() {
            return Err(Error::NoFieldNames);
        }

        let available_fields: Vec<String> = source
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();

        // Get the first field to determine the element type
        let first_field_name = &self.field_names[0];
        let first_array = source.column_by_name(first_field_name).ok_or_else(|| {
            Error::MissingStructField {
                field_name: first_field_name.clone(),
                struct_fields: available_fields.clone(),
            }
        })?;
        let element_type = first_array.data_type().clone();

        // Collect all field arrays, ensuring they all have the same type
        let mut field_arrays = Vec::new();
        field_arrays.push(first_array);

        for field_name in &self.field_names[1..] {
            let array = source.column_by_name(field_name).ok_or_else(|| {
                Error::MissingStructField {
                    field_name: field_name.clone(),
                    struct_fields: available_fields.clone(),
                }
            })?;

            // Verify type consistency
            if array.data_type() != &element_type {
                return Err(Error::InconsistentFieldTypes {
                    field_name: field_name.clone(),
                    actual_type: array.data_type().clone(),
                    reference_field: first_field_name.clone(),
                    expected_type: element_type.clone(),
                });
            }

            field_arrays.push(array);
        }

        let len = source.len();
        let list_size = self.field_names.len();

        // Build the flattened values array by concatenating field arrays
        let mut concatenated_arrays = Vec::new();
        for row_idx in 0..len {
            for field_array in &field_arrays {
                concatenated_arrays.push(field_array.slice(row_idx, 1));
            }
        }

        // Concatenate all slices into a single array
        let refs: Vec<&dyn Array> = concatenated_arrays.iter().map(|a| a.as_ref()).collect();
        let values = arrow::compute::concat(&refs)?;

        let field = Arc::new(Field::new("item", element_type, true));

        Ok(FixedSizeListArray::new(
            field,
            list_size as i32,
            values,
            None, // No outer nulls
        ))
    }
}

/// Unwraps a struct array within a list and extracts a field from it.
///
/// This is useful for nested structures like `List<Struct<field: List<T>>>`,
/// where you want to extract the inner field directly.
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

    fn transform(&self, source: &ListArray) -> Result<ListArray, Error> {
        // Get the struct array from the list values
        let values = source.values();
        let struct_array = values
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::ExpectedStructInList {
                actual: values.data_type().clone(),
            })?;

        // Extract the field
        let field_array = struct_array.column_by_name(&self.field_name).ok_or_else(|| {
            let available_fields = struct_array
                .fields()
                .iter()
                .map(|f| f.name().clone())
                .collect();
            Error::FieldNotFound {
                field_name: self.field_name.clone(),
                available_fields,
            }
        })?;

        // Downcast to ListArray
        field_array
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| Error::FieldTypeMismatch {
                field_name: self.field_name.clone(),
                expected_type: "ListArray".to_string(),
                actual_type: field_array.data_type().clone(),
            })
            .map(Clone::clone)
    }
}

/// Maps a function over each element in a Float64Array.
///
/// Applies the given function to each non-null element, preserving null values.
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

    fn transform(&self, source: &Float64Array) -> Result<Float64Array, Error> {
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

/// Casts a primitive array from one type to another using Arrow's type casting.
///
/// This uses Arrow's `cast` function for primitive type conversions. Null values are preserved.
/// Some conversions may be lossy (e.g., f64 to f32, i64 to i32).
///
/// The source and target types are specified via generic parameters to maintain type safety.
/// The target data type is automatically deduced from the target's `ArrowPrimitiveType`.
#[derive(Clone)]
pub struct Cast<S, T> {
    _phantom: std::marker::PhantomData<(S, T)>,
}

impl<S, T> Cast<PrimitiveArray<S>, PrimitiveArray<T>>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
{
    /// Create a new cast transformation.
    ///
    /// The target data type is automatically deduced from the target primitive type `T`.
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S, T> Default for Cast<PrimitiveArray<S>, PrimitiveArray<T>>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S, T> Transform for Cast<PrimitiveArray<S>, PrimitiveArray<T>>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
{
    type Source = PrimitiveArray<S>;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &PrimitiveArray<S>) -> Result<PrimitiveArray<T>, Error> {
        let source_ref: &dyn Array = source;
        let target_type = T::DATA_TYPE;
        let casted = cast(source_ref, &target_type)?;

        casted
            .as_any()
            .downcast_ref::<PrimitiveArray<T>>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: std::any::type_name::<PrimitiveArray<T>>().to_string(),
                actual: casted.data_type().clone(),
                context: "cast result".to_string(),
            })
            .map(Clone::clone)
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
            .then(MapList::new(StructToFixedList::new(["x", "y"])));

        let result: ListArray = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapListStructField::new("poses")
            .then(MapList::new(StructToFixedList::new(["x", "y"])))
            .then(MapList::new(MapFixedSizeList::new(MapFloat64::new(|x| x + 1.0))));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();

        let pipeline = UnwrapListStructField::new("poses")
            .then(MapList::new(StructToFixedList::new(["x", "y"])))
            .then(MapList::new(MapFixedSizeList::new(
                Cast::<Float64Array, Float32Array>::new(),
            )));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }
}
