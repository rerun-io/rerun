use arrow::array::{Array, ArrayRef};
use arrow::datatypes::DataType;

use crate::{DeserializationError, DeserializationResult, ResultExt as _};

/// Move an arrow array into an [`ArrayRef`].
pub fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
    std::sync::Arc::new(t) as ArrayRef
}

/// Downcast an arrow array to a concrete array type, without having to go via `Any`.
///
/// Used by the generated deserializers.
/// See also [`ArrowArrayDowncastRef`](https://docs.rs/re_arrow_util), which is the same thing
/// for code that wants an `ArrowError` instead of a [`DeserializationError`].
pub trait ArrayTryCast<'a>: 'a {
    /// Downcast to `T`, or fail with a datatype mismatch error.
    ///
    /// `expected` is only called on failure, to build the error message.
    /// It is a function pointer rather than a value so that we don't pay for
    /// building a [`DataType`] on the happy path.
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T>;
}

impl<'a> ArrayTryCast<'a> for &'a dyn Array {
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T> {
        self.as_any().downcast_ref::<T>().ok_or_else(|| {
            DeserializationError::datatype_mismatch(expected(), self.data_type().clone())
        })
    }
}

impl<'a> ArrayTryCast<'a> for &'a ArrayRef {
    fn try_cast<T: Array + 'static>(
        self,
        expected: fn() -> DataType,
    ) -> DeserializationResult<&'a T> {
        let array: &'a dyn Array = &**self;
        array.try_cast(expected)
    }
}

/// Fails if any entry of `array` is null.
///
/// Used by the generated deserializers for anything non-nullable:
/// there is no value to deserialize a null into.
///
/// Returns the array itself, so that it can be used in the middle of a chain.
pub fn err_on_nulls<'a>(
    array: &'a dyn Array,
    context: &'static str,
) -> DeserializationResult<&'a dyn Array> {
    if 0 < array.null_count() {
        return Err(DeserializationError::missing_data()).with_context(context);
    }
    Ok(array)
}
