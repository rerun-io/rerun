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
        self.0.slice(range.start, range.len()).into()
    }

    /// Returns the bytes of a serialized blob batch without copying it.
    ///
    /// Returns `None` if the serialized component batch didn't have the expected shape.
    pub fn serialized_blob_as_slice(
        serialized_blob: &re_types_core::SerializedComponentBatch,
    ) -> Option<&[u8]> {
        let blob_list_array = serialized_blob
            .array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()?;
        let blob_data = blob_list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()?;

        Some(blob_data.values().inner().as_slice())
    }
}

impl Eq for Blob {}

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
    type Target = arrow::buffer::ScalarBuffer<u8>;

    #[inline]
    fn deref(&self) -> &arrow::buffer::ScalarBuffer<u8> {
        &self.0
    }
}
