//! Type-safe, composable transformations for Arrow arrays.
//!
//! This crate provides composable transformations for Arrow arrays.
//! Transformations are composable operations that convert one array type to another,
//! preserving structural properties like row counts and null handling.
//!
//! These transformations serve as building blocks for user-defined functions (UDFs)
//! in query engines like `DataFusion`, as well as SDK features like lenses.

use std::marker::PhantomData;
use std::num::TryFromIntError;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, ArrowPrimitiveType, FixedSizeListArray, GenericBinaryArray, GenericListArray,
    ListArray, OffsetSizeTrait, PrimitiveArray, StructArray,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;

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

    #[error("Offset overflow: cannot fit {actual} into {expected_type}")]
    OffsetOverflow {
        actual: usize,
        expected_type: &'static str,
    },

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
        let values = re_arrow_util::concat_arrays(&refs)?;

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
            re_arrow_util::concat_arrays(&refs)?
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

/// Converts binary arrays to list arrays where each binary element becomes a list of `u8`.
///
/// The underlying bytes buffer is reused, making this transformation almost zero-copy.
#[derive(Clone, Debug, Default)]
pub struct BinaryToListUInt8<O1: OffsetSizeTrait, O2: OffsetSizeTrait = O1> {
    _from_offset: PhantomData<O1>,
    _to_offset: PhantomData<O2>,
}

impl<O1: OffsetSizeTrait, O2: OffsetSizeTrait> BinaryToListUInt8<O1, O2> {
    /// Create a new transformation to convert a binary array to a list array of `u8` arrays.
    pub fn new() -> Self {
        Default::default()
    }
}

impl<O1: OffsetSizeTrait, O2: OffsetSizeTrait> Transform for BinaryToListUInt8<O1, O2> {
    type Source = GenericBinaryArray<O1>;
    type Target = GenericListArray<O2>;

    fn transform(&self, source: &GenericBinaryArray<O1>) -> Result<Self::Target, Error> {
        use arrow::array::UInt8Array;
        use arrow::buffer::ScalarBuffer;

        let scalar_buffer: ScalarBuffer<u8> = ScalarBuffer::from(source.values().clone());
        let uint8_array = UInt8Array::new(scalar_buffer, None);

        // Convert from O1 to O2. Most offset buffers will be small in real-world
        // examples, so we're fine copying them.
        //
        // This could be true zero copy if Rust had specialization.
        // More info: https://std-dev-guide.rust-lang.org/policy/specialization.html
        let old_offsets = source.offsets().iter();
        let new_offsets: Result<Vec<O2>, Error> = old_offsets
            .map(|&offset| {
                let offset_usize = offset.as_usize();
                O2::from_usize(offset_usize).ok_or_else(|| Error::OffsetOverflow {
                    actual: offset_usize,
                    expected_type: std::any::type_name::<O2>(),
                })
            })
            .collect();
        let offsets = arrow::buffer::OffsetBuffer::new(new_offsets?.into());

        let list = Self::Target::new(
            Arc::new(Field::new_list_field(DataType::UInt8, false)),
            offsets,
            Arc::new(uint8_array),
            source.nulls().cloned(),
        );

        Ok(list)
    }
}
