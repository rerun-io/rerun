//! Type-safe, composable transformations for Arrow arrays.

use std::num::TryFromIntError;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, ArrowPrimitiveType, FixedSizeListArray, ListArray, PrimitiveArray, StructArray,
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
    #[error("Field '{field_name}' not found. Available fields: [{}]", available_fields.join(", "))]
    FieldNotFound {
        field_name: String,
        available_fields: Vec<String>,
    },

    #[error("Field '{field_name}' has wrong type: expected {expected_type}, got {actual_type:?}")]
    FieldTypeMismatch {
        field_name: String,
        expected_type: String,
        actual_type: DataType,
    },

    #[error("Type mismatch in {context}: expected {expected}, got {actual:?}")]
    TypeMismatch {
        expected: String,
        actual: DataType,
        context: String,
    },

    #[error("Struct is missing required field '{field_name}'. Available fields: [{}]", struct_fields.join(", "))]
    MissingStructField {
        field_name: String,
        struct_fields: Vec<String>,
    },

    #[error("List contains unexpected value type: expected {expected}, got {actual:?}")]
    UnexpectedListValueType { expected: String, actual: DataType },

    #[error("Fixed-size list contains unexpected value type: expected {expected}, got {actual:?}")]
    UnexpectedFixedSizeListValueType { expected: String, actual: DataType },

    #[error("Expected list to contain struct values, but got {actual:?}")]
    ExpectedStructInList { actual: DataType },

    #[error(
        "Field '{field_name}' has type {actual_type:?}, but expected {expected_type:?} (inferred from field '{reference_field}')"
    )]
    InconsistentFieldTypes {
        field_name: String,
        actual_type: DataType,
        reference_field: String,
        expected_type: DataType,
    },

    #[error("Cannot create fixed-size list with {actual} fields: {err}")]
    InvalidNumberOfFields { actual: usize, err: TryFromIntError },

    #[error("At least one field name is required")]
    NoFieldNames,

    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

/// A transformation that converts one Arrow array type to another.
///
/// Transformations are read-only operations that may fail (e.g., missing field, type mismatch).
/// They can be composed using the `then` method to create complex transformation pipelines.
pub trait Transform {
    /// The source array type.
    type Source: Array;

    /// The target array type.
    type Target: Array;

    /// Apply the transformation to the source array.
    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error>;

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
                let available_fields = source.fields().iter().map(|f| f.name().clone()).collect();
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
        let downcast =
            values
                .as_any()
                .downcast_ref::<S>()
                .ok_or_else(|| Error::UnexpectedListValueType {
                    expected: std::any::type_name::<S>().to_owned(),
                    actual: values.data_type().clone(),
                })?;

        let transformed = self.transform.transform(downcast)?;
        let new_field = Arc::new(Field::new(
            "item",
            transformed.data_type().clone(),
            transformed.is_nullable(),
        ));

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
                expected: std::any::type_name::<S>().to_owned(),
                actual: values.data_type().clone(),
            }
        })?;

        let transformed = self.transform.transform(downcast)?;
        let field = Arc::new(Field::new_list_field(
            transformed.data_type().clone(),
            transformed.is_nullable(),
        ));
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

        let available_fields: Vec<String> =
            source.fields().iter().map(|f| f.name().clone()).collect();

        // Get the first field to determine the element type
        let first_field_name = &self.field_names[0];
        let first_array =
            source
                .column_by_name(first_field_name)
                .ok_or_else(|| Error::MissingStructField {
                    field_name: first_field_name.clone(),
                    struct_fields: available_fields.clone(),
                })?;
        let element_type = first_array.data_type().clone();

        // Collect all field arrays, ensuring they all have the same type
        let mut field_arrays = Vec::new();
        field_arrays.push(first_array);

        for field_name in &self.field_names[1..] {
            let array =
                source
                    .column_by_name(field_name)
                    .ok_or_else(|| Error::MissingStructField {
                        field_name: field_name.clone(),
                        struct_fields: available_fields.clone(),
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

        // Build the flattened values array by concatenating field arrays
        let mut concatenated_arrays = Vec::new();
        for row_idx in 0..source.len() {
            for field_array in &field_arrays {
                concatenated_arrays.push(field_array.slice(row_idx, 1));
            }
        }

        // Concatenate all slices into a single array
        let refs: Vec<&dyn Array> = concatenated_arrays.iter().map(|a| a.as_ref()).collect();
        let values = crate::concat_arrays(&refs)?;

        let field = Arc::new(Field::new("item", element_type, true));

        let list_size = self.field_names.len();
        let list_size = i32::try_from(list_size).map_err(|err| Error::InvalidNumberOfFields {
            actual: list_size,
            err,
        })?;
        Ok(FixedSizeListArray::new(
            field, list_size, values, None, // No outer nulls
        ))
    }
}

/// Maps a function over each element in a primitive array.
///
/// Applies the given function to each non-null element, preserving null values.
/// Works with any Arrow primitive type.
#[derive(Clone)]
pub struct MapPrimitive<S, F, T = S>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    f: F,
    _phantom_source: std::marker::PhantomData<S>,
    _phantom_target: std::marker::PhantomData<T>,
}

impl<S, F, T> MapPrimitive<S, F, T>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    /// Create a new mapper that applies the given function to each element.
    pub fn new(f: F) -> Self {
        Self {
            f,
            _phantom_source: std::marker::PhantomData,
            _phantom_target: std::marker::PhantomData,
        }
    }
}

impl<S, F, T> Transform for MapPrimitive<S, F, T>
where
    S: ArrowPrimitiveType,
    T: ArrowPrimitiveType,
    F: Fn(S::Native) -> T::Native,
{
    type Source = PrimitiveArray<S>;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &PrimitiveArray<S>) -> Result<PrimitiveArray<T>, Error> {
        let result: PrimitiveArray<T> = source.iter().map(|opt| opt.map(|v| (self.f)(v))).collect();
        Ok(result)
    }
}

/// Replaces null values in a primitive array with a specified default value.
///
/// All null entries in the source array will be replaced with the provided value,
/// while non-null entries remain unchanged.
#[derive(Clone)]
pub struct ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    default_value: T::Native,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    /// Create a new null replacer with the given default value.
    pub fn new(default_value: T::Native) -> Self {
        Self {
            default_value,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Transform for ReplaceNull<T>
where
    T: ArrowPrimitiveType,
{
    type Source = PrimitiveArray<T>;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &PrimitiveArray<T>) -> Result<PrimitiveArray<T>, Error> {
        let result: PrimitiveArray<T> = source
            .iter()
            .map(|opt| Some(opt.unwrap_or(self.default_value)))
            .collect();
        Ok(result)
    }
}

/// Casts a primitive array from one type to another using Arrow's type casting.
///
/// This uses Arrow's `cast` function for primitive type conversions. Null values are preserved.
/// Some conversions may be lossy (e.g., f64 to f32, i64 to i32).
///
/// The source and target types are specified via generic parameters to maintain type safety.
/// The target data type is automatically deduced from the target's `ArrowPrimitiveType`.
#[derive(Clone, Default)]
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
                expected: std::any::type_name::<PrimitiveArray<T>>().to_owned(),
                actual: casted.data_type().clone(),
                context: "cast result".to_owned(),
            })
            .cloned()
    }
}

/// Flattens a nested list array by one level.
///
/// Takes `List<List<T>>` and flattens it to `List<T>` by concatenating all inner lists
/// within each outer list row.
///
/// # Example
///
/// - `[[1, 2], [3, 4]]` → `[1, 2, 3, 4]` (concatenates inner lists)
/// - `[[5], [6, 7, 8]]` → `[5, 6, 7, 8]`
/// - `[[]]` → `[]` (empty inner list produces empty result)
/// - `null` → `null` (null rows are preserved)
#[derive(Clone, Debug, Default)]
pub struct Flatten;

impl Flatten {
    /// Create a new flatten transformation.
    pub fn new() -> Self {
        Self
    }
}

impl Transform for Flatten {
    type Source = ListArray;
    type Target = ListArray;

    fn transform(&self, source: &ListArray) -> Result<ListArray, Error> {
        let values = source.values();

        // The values should be a ListArray that we want to flatten
        let inner_list =
            values
                .as_any()
                .downcast_ref::<ListArray>()
                .ok_or_else(|| Error::TypeMismatch {
                    expected: "List".to_owned(),
                    actual: values.data_type().clone(),
                    context: "Flatten expects List<List<T>>".to_owned(),
                })?;

        let outer_offsets = source.offsets();
        let inner_offsets = inner_list.offsets();
        let inner_values = inner_list.values();

        // Fast path: check if each outer list contains at most one inner list
        // In this case, we can just unwrap directly
        let mut is_trivial = true;
        for outer_row_idx in 0..source.len() {
            if !source.is_null(outer_row_idx) {
                let outer_start = outer_offsets[outer_row_idx] as usize;
                let outer_end = outer_offsets[outer_row_idx + 1] as usize;
                let count = outer_end - outer_start;
                if count > 1 {
                    is_trivial = false;
                    break;
                }
            }
        }

        if is_trivial {
            // Each outer list has 0 or 1 inner lists - just unwrap
            // Map outer offsets through inner offsets
            let mut new_offsets = Vec::with_capacity(source.len() + 1);

            for outer_row_idx in 0..=source.len() {
                let outer_idx = outer_offsets[outer_row_idx] as usize;
                let inner_offset = inner_offsets[outer_idx];
                new_offsets.push(inner_offset);
            }

            let field = Arc::new(Field::new("item", inner_values.data_type().clone(), true));
            let offsets = arrow::buffer::OffsetBuffer::new(new_offsets.into());

            return Ok(ListArray::new(
                field,
                offsets,
                inner_values.clone(),
                source.nulls().cloned(),
            ));
        }

        // General case: build new offsets and collect value ranges
        let mut new_offsets = Vec::with_capacity(source.len() + 1);
        new_offsets.push(0i32);

        let mut current_offset = 0i32;

        // Collect ranges of values to copy (as (start, length) pairs)
        let mut value_ranges: Vec<(i32, i32)> = Vec::new();

        for outer_row_idx in 0..source.len() {
            if source.is_null(outer_row_idx) {
                new_offsets.push(current_offset);
                continue;
            }

            let outer_start = outer_offsets[outer_row_idx];
            let outer_end = outer_offsets[outer_row_idx + 1];

            for inner_idx in outer_start..outer_end {
                let inner_idx = inner_idx as usize;
                if !inner_list.is_null(inner_idx) {
                    let inner_start = inner_offsets[inner_idx];
                    let inner_end = inner_offsets[inner_idx + 1];
                    let length = inner_end - inner_start;

                    if length > 0 {
                        // Try to merge with previous range if contiguous
                        if let Some((last_start, last_len)) = value_ranges.last_mut() {
                            if *last_start + *last_len == inner_start {
                                *last_len += length;
                            } else {
                                value_ranges.push((inner_start, length));
                            }
                        } else {
                            value_ranges.push((inner_start, length));
                        }
                        current_offset += length;
                    }
                }
            }

            new_offsets.push(current_offset);
        }

        // Build flattened values by slicing larger contiguous chunks
        let flattened_values = if value_ranges.is_empty() {
            inner_values.slice(0, 0)
        } else if value_ranges.len() == 1 {
            // Single contiguous range - just slice once
            let (start, length) = value_ranges[0];
            inner_values.slice(start as usize, length as usize)
        } else {
            // Multiple ranges - slice and concatenate
            let slices: Vec<_> = value_ranges
                .iter()
                .map(|&(start, length)| inner_values.slice(start as usize, length as usize))
                .collect();
            let refs: Vec<&dyn Array> = slices.iter().map(|a| a.as_ref()).collect();
            crate::concat_arrays(&refs)?
        };

        let field = Arc::new(Field::new("item", inner_values.data_type().clone(), true));
        let offsets = arrow::buffer::OffsetBuffer::new(new_offsets.into());

        Ok(ListArray::new(
            field,
            offsets,
            flattened_values,
            source.nulls().cloned(),
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use arrow::{
        array::{
            ArrayRef, Float32Array, Float64Array, Float64Builder, ListArray, ListBuilder,
            RecordBatch, RecordBatchOptions, StructBuilder,
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
        struct_val
            .field_builder::<ListBuilder<StructBuilder>>(0)
            .unwrap()
            .append(true);
        struct_val.append(true);
        column_builder.append(true);

        // Row 2:
        column_builder.append(false);

        // Row 3:
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

        // Row 4:
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

        column_builder.finish()
    }

    #[test]
    fn simple() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = MapList::new(GetField::new("poses"))
            .then(Flatten::new())
            .then(MapList::new(StructToFixedList::new(["x", "y"])));

        let result: ListArray = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("simple", format!("{}", DisplayRB::from(result.clone())));
    }

    #[test]
    fn add_one_to_leaves() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = MapList::new(GetField::new("poses"))
            .then(Flatten::new())
            .then(MapList::new(StructToFixedList::new(["x", "y"])))
            .then(MapList::new(MapFixedSizeList::new(MapPrimitive::<
                arrow::datatypes::Float64Type,
                _,
            >::new(
                |x| x + 1.0
            ))));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("add_one_to_leaves", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn convert_to_f32() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = MapList::new(GetField::new("poses"))
            .then(Flatten::new())
            .then(MapList::new(StructToFixedList::new(["x", "y"])))
            .then(MapList::new(MapFixedSizeList::new(Cast::<
                Float64Array,
                Float32Array,
            >::new())));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("convert_to_f32", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn replace_nulls() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = MapList::new(GetField::new("poses"))
            .then(Flatten::new())
            .then(MapList::new(StructToFixedList::new(["x", "y"])))
            .then(MapList::new(MapFixedSizeList::new(ReplaceNull::<
                arrow::datatypes::Float64Type,
            >::new(
                1337.0
            ))));

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!("replace_nulls", format!("{}", DisplayRB::from(result)));
    }

    #[test]
    fn test_flatten_single_element() {
        let array = create_nasty_component_column();
        println!("{}", DisplayRB::from(array.clone()));

        let pipeline = MapList::new(GetField::new("poses")).then(Flatten::new());

        let result = pipeline.transform(&array).unwrap();

        insta::assert_snapshot!(
            "flatten_single_element",
            format!("{}", DisplayRB::from(result))
        );
    }

    #[test]
    fn test_flatten_multiple_elements() {
        let inner_builder = ListBuilder::new(arrow::array::Int32Builder::new());
        let mut outer_builder = ListBuilder::new(inner_builder);

        // Row 0: [[1, 2], [3, 4]] -> should flatten to [1, 2, 3, 4]
        outer_builder.values().values().append_value(1);
        outer_builder.values().values().append_value(2);
        outer_builder.values().append(true);
        outer_builder.values().values().append_value(3);
        outer_builder.values().values().append_value(4);
        outer_builder.values().append(true);
        outer_builder.append(true);

        // Row 1: [[5], [6, 7, 8]] -> should flatten to [5, 6, 7, 8]
        outer_builder.values().values().append_value(5);
        outer_builder.values().append(true);
        outer_builder.values().values().append_value(6);
        outer_builder.values().values().append_value(7);
        outer_builder.values().values().append_value(8);
        outer_builder.values().append(true);
        outer_builder.append(true);

        // Row 2: [[]] -> should flatten to []
        outer_builder.values().append(true);
        outer_builder.append(true);

        // Row 3: [[], [9]] -> should flatten to [9]
        outer_builder.values().append(true);
        outer_builder.values().values().append_value(9);
        outer_builder.values().append(true);
        outer_builder.append(true);

        // Row 4: null -> should remain null
        outer_builder.append(false);

        // Row 5: [[10, 11]] -> should flatten to [10, 11]
        outer_builder.values().values().append_value(10);
        outer_builder.values().values().append_value(11);
        outer_builder.values().append(true);
        outer_builder.append(true);

        // Row 6: [[32], [33, 34], [], null] -> should flatten to [32, 33, 34]
        outer_builder.values().values().append_value(32);
        outer_builder.values().append(true);
        outer_builder.values().values().append_value(33);
        outer_builder.values().values().append_value(34);
        outer_builder.values().append(true);
        outer_builder.values().append(true);
        outer_builder.values().append(false);
        outer_builder.append(true);

        let list_of_lists = outer_builder.finish();

        println!("{}", DisplayRB::from(list_of_lists.clone()));

        let result = Flatten::new().transform(&list_of_lists).unwrap();

        insta::assert_snapshot!(
            "flatten_multiple_elements",
            format!("{}", DisplayRB::from(result))
        );
    }
}
