//! Transforms that cast arrays to different types.

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, ArrowPrimitiveType, PrimitiveArray};
use arrow::compute::cast;
use arrow::datatypes::Field;

use crate::{Error, Transform};

/// Casts a primitive array from one type to another using Arrow's type casting.
///
/// This uses Arrow's `cast` function for primitive type conversions. Null values are preserved.
/// Some conversions may be lossy (e.g., f64 to f32, i64 to i32).
///
/// The source and target types are specified via generic parameters to maintain type safety.
/// The target data type is automatically deduced from the target's `ArrowPrimitiveType`.
#[derive(Clone, Default)]
pub struct PrimitiveCast<S, T> {
    _phantom: std::marker::PhantomData<(S, T)>,
}

impl<S, T> PrimitiveCast<PrimitiveArray<S>, PrimitiveArray<T>>
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

impl<S, T> Transform for PrimitiveCast<PrimitiveArray<S>, PrimitiveArray<T>>
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

        DowncastRef::<T>::new().transform(&casted)
    }
}

/// Downcasts an `ArrayRef` to a `PrimitiveArray<T>` if the inner value is of type `T`.
#[derive(Clone, Default)]
pub struct DowncastRef<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> DowncastRef<T> {
    /// Create a new downcast transformation.
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> Transform for DowncastRef<T>
where
    T: ArrowPrimitiveType,
{
    type Source = ArrayRef;
    type Target = PrimitiveArray<T>;

    fn transform(&self, source: &ArrayRef) -> Result<PrimitiveArray<T>, Error> {
        source
            .as_any()
            .downcast_ref::<PrimitiveArray<T>>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: std::any::type_name::<PrimitiveArray<T>>().to_owned(),
                actual: source.data_type().clone(),
                context: "downcast_ref".to_owned(),
            })
            .cloned()
    }
}

/// Casts a `ListArray` to a `FixedSizeListArray` with the specified value length.
///
/// The source `ListArray` must have lists of exactly that length (or null).
#[derive(Clone)]
pub struct ListToFixedSizeList {
    value_length: i32,
}

impl ListToFixedSizeList {
    /// Create a new `ListToFixedSizeList` transformation with an expected value length.
    pub fn new(value_length: i32) -> Self {
        Self { value_length }
    }
}

impl Transform for ListToFixedSizeList {
    type Source = arrow::array::ListArray;
    type Target = arrow::array::FixedSizeListArray;

    fn transform(&self, source: &Self::Source) -> Result<Self::Target, Error> {
        // Check that each list has exactly the expected length (or is null).
        let offsets = source.value_offsets();
        let expected_length = self.value_length as usize;
        for list_index in 0..source.len() {
            if source.is_valid(list_index) {
                let start = offsets[list_index] as usize;
                let end = offsets[list_index + 1] as usize;
                let list_length = end - start;
                if list_length != expected_length {
                    return Err(Error::UnexpectedListValueLength {
                        expected: expected_length,
                        actual: list_length,
                    });
                }
            }
        }

        // Build the FixedSizeListArray.
        let field = Arc::new(Field::new_list_field(
            source.value_type().clone(),
            source.is_nullable(),
        ));
        Ok(arrow::array::FixedSizeListArray::try_new(
            field,
            self.value_length,
            source.values().clone(),
            source.nulls().cloned(),
        )?)
    }
}
