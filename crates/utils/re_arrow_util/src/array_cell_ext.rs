use std::sync::Arc;

use arrow::array::{Array, ArrayRef, Int32DictionaryArray, ListArray, NullArray};
use arrow::datatypes::DataType;

/// A slice of an array that carries array vs. scalar semantics, which is typically useful for user
/// representations (UI, text output, etc.).
///
/// See [`ArrayCellExt::get`].
#[derive(Debug, Clone)]
pub enum ArrayCell {
    /// This cell is semantically an array. It may have any length.
    Array(ArrayRef),

    /// This cell is semantically a scalar and is guaranteed to have a length of 1.
    Scalar(ArrayRef),
}

impl ArrayCell {
    pub fn inner(&self) -> &'_ dyn Array {
        match self {
            Self::Array(array) | Self::Scalar(array) => array.as_ref(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Array(array) | Self::Scalar(array) => array.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Array(array) | Self::Scalar(array) => array.is_empty(),
        }
    }
}

/// Same as [`ArrayCell`], but with references instead of ref-counted values.
pub enum ArrayCellRef<'a> {
    Array(&'a dyn Array),
    Scalar(&'a dyn Array),
}

impl<'a> From<&'a ArrayCell> for ArrayCellRef<'a> {
    fn from(value: &'a ArrayCell) -> Self {
        match value {
            ArrayCell::Array(array) => Self::Array(array.as_ref()),
            ArrayCell::Scalar(array) => Self::Scalar(array.as_ref()),
        }
    }
}

//TODO: remove these, we should force callsites to be explicit

impl<'a> From<&'a ArrayRef> for ArrayCellRef<'a> {
    fn from(value: &'a ArrayRef) -> Self {
        Self::Array(value.as_ref())
    }
}

impl<'a> From<&'a dyn Array> for ArrayCellRef<'a> {
    fn from(value: &'a dyn Array) -> Self {
        Self::Array(value)
    }
}

/// Extension trait for [`Array`] to provide a convenient way to get a [`ArrayCell`] from an index.
pub trait ArrayCellExt {
    /// Get a single element from this array.
    ///
    /// If the array is nested, returns a [`ArrayCell::Array`] with the nested type. Otherwise,
    /// return a [`ArrayCell::Scalar`] with the indexed primitive value.
    fn get(&self, index: usize) -> Option<ArrayCell>;
}

impl<T> ArrayCellExt for T
where
    T: Array + ?Sized,
{
    fn get(&self, index: usize) -> Option<ArrayCell> {
        if index >= self.len() {
            return None;
        }

        match self.data_type() {
            DataType::List(_) => {
                let list_array = self
                    .as_any()
                    .downcast_ref::<ListArray>()
                    .expect("the data type was checked");

                //TODO: is this sufficiently handling nulls?
                if list_array.is_null(index) {
                    Some(ArrayCell::Scalar(Arc::new(NullArray::new(0))))
                } else {
                    Some(ArrayCell::Array(list_array.value(index)))
                }
            }

            //TODO(ab): support all dictionary types
            DataType::Dictionary(key_data_type, _)
                if key_data_type.as_ref() == &DataType::Int32 =>
            {
                let dict_array = self
                    .as_any()
                    .downcast_ref::<Int32DictionaryArray>()
                    .expect("the data type was checked");

                let key = dict_array.key(index)?;
                let cell = dict_array.values().slice(key, 1);

                Some(ArrayCell::Array(cell))
            }

            //TODO: list all types explicitly
            //TODO: handle more container types, dictionary, etc.?
            _ => Some(ArrayCell::Scalar(self.slice(index, 1))),
        }
    }
}

impl ArrayCellExt for ArrayCell {
    fn get(&self, index: usize) -> Option<ArrayCell> {
        match self {
            Self::Array(array) => array.get(index),
            Self::Scalar(_) => None,
        }
    }
}

//TODO: tests
