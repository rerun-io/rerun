use arrow::{array::Array as _, buffer::ScalarBuffer};

use super::Blob;

impl Blob {
    /// Returns a new [`Blob`] that is a slice of this buffer starting at `offset`.
    ///
    /// Doing so allows the same memory region to be shared between buffers.
    /// Note that you can beforehand `clone` the [`Blob`] to get a new buffer that shares the same memory.
    ///
    /// # Panics
    /// Panics iff `offset + length` is larger than `len`.
    #[inline]
    pub fn sliced(self, range: std::ops::Range<usize>) -> Self {
        self.0.slice_with_length(range.start, range.len()).into()
    }

    /// Returns the bytes of a serialized blob batch without copying it.
    ///
    /// Returns `None` if the serialized component batch didn't have the expected type or shape.
    pub fn serialized_blob_as_slice(
        serialized_blob: &re_types_core::SerializedComponentBatch,
    ) -> Option<&[u8]> {
        Self::binary_array_as_slice(&serialized_blob.array)
    }

    /// Returns the bytes of a serialized blob batch without copying it.
    ///
    /// Returns `None` if the serialized component batch didn't have the expected type or shape.
    pub fn binary_array_as_slice(array: &std::sync::Arc<dyn arrow::array::Array>) -> Option<&[u8]> {
        if let Some(blob_data) = array.as_any().downcast_ref::<arrow::array::BinaryArray>() {
            if blob_data.len() == 1 {
                return Some(blob_data.value(0));
            }
        }

        if let Some(blob_data) = array
            .as_any()
            .downcast_ref::<arrow::array::LargeBinaryArray>()
        {
            if blob_data.len() == 1 {
                return Some(blob_data.value(0));
            }
        }

        None
    }
}

impl Eq for Blob {}

impl From<ScalarBuffer<u8>> for Blob {
    fn from(buffer: ScalarBuffer<u8>) -> Self {
        Self(buffer.into())
    }
}

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }
}

impl From<&[u8]> for Blob {
    fn from(bytes: &[u8]) -> Self {
        Self::from(bytes.to_vec())
    }
}

impl std::ops::Deref for Blob {
    type Target = arrow::buffer::Buffer;

    #[inline]
    fn deref(&self) -> &arrow::buffer::Buffer {
        &self.0
    }
}
