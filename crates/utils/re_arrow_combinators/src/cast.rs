//! Transforms that cast arrays to different types.

use arrow::{
    array::{Array, ArrayRef, ArrowPrimitiveType, PrimitiveArray},
    compute::cast,
};

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
