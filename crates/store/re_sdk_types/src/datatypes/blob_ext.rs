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

impl From<arrow::buffer::Buffer> for Blob {
    fn from(buffer: arrow::buffer::Buffer) -> Self {
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
    type Target = arrow::buffer::ScalarBuffer<u8>;

    #[inline]
    fn deref(&self) -> &arrow::buffer::ScalarBuffer<u8> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::Array as _;
    use re_types_core::Loggable as _;

    use super::*;

    #[test]
    fn test_single_blob_serialization() {
        // Test the optimized path for single blob (common case for images)
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let blob = Blob::from(data.clone());

        // Serialize a single blob
        let blobs = vec![Some(blob)];
        let array = Blob::to_arrow_opt(blobs).unwrap();

        // Verify it's a ListArray
        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");

        // Verify the data
        assert_eq!(list_array.len(), 1);
        let inner = list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()
            .expect("Inner should be UInt8Array");

        let inner_data: Vec<u8> = inner.values().iter().copied().collect();
        assert_eq!(inner_data, data);
    }

    #[test]
    fn test_multiple_blob_serialization() {
        // Test the concat path for multiple blobs
        let data1 = vec![1u8, 2, 3, 4];
        let data2 = vec![5u8, 6, 7, 8];
        let blob1 = Blob::from(data1.clone());
        let blob2 = Blob::from(data2.clone());

        // Serialize multiple blobs
        let blobs = vec![Some(blob1), Some(blob2)];
        let array = Blob::to_arrow_opt(blobs).unwrap();

        // Verify it's a ListArray
        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");

        // Verify the data
        assert_eq!(list_array.len(), 2);

        // Verify first blob
        let offsets = list_array.offsets();
        let inner = list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()
            .expect("Inner should be UInt8Array");

        let start1 = offsets[0] as usize;
        let end1 = offsets[1] as usize;
        let slice1: Vec<u8> = inner.values()[start1..end1].to_vec();
        assert_eq!(slice1, data1);

        // Verify second blob
        let start2 = offsets[1] as usize;
        let end2 = offsets[2] as usize;
        let slice2: Vec<u8> = inner.values()[start2..end2].to_vec();
        assert_eq!(slice2, data2);
    }

    #[test]
    fn test_blob_serialization_roundtrip() {
        // Test that single and multiple blobs can roundtrip through serialization
        let data = vec![42u8; 1024]; // Simulate a small image
        let blob = Blob::from(data.clone());

        // Single blob roundtrip
        let blobs_in = vec![Some(blob.clone())];
        let array = Blob::to_arrow_opt(blobs_in).unwrap();
        let blobs_out = Blob::from_arrow_opt(&*array).unwrap();

        assert_eq!(blobs_out.len(), 1);
        assert_eq!(blobs_out[0].as_ref().unwrap().0.as_ref(), data.as_slice());
    }

    #[test]
    fn test_empty_blob() {
        // Test edge case: empty blob
        let blob = Blob::from(vec![]);
        let blobs = vec![Some(blob)];
        let array = Blob::to_arrow_opt(blobs).unwrap();

        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");

        assert_eq!(list_array.len(), 1);
        let offsets = list_array.offsets();
        assert_eq!(offsets[1] - offsets[0], 0); // Empty blob
    }

    #[test]
    fn test_large_single_blob() {
        // Test the optimized path with a large blob (like a typical image)
        let data = vec![128u8; 1920 * 1080 * 3]; // Simulated Full HD RGB image
        let blob = Blob::from(data.clone());

        let blobs = vec![Some(blob)];
        let array = Blob::to_arrow_opt(blobs).unwrap();

        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");

        assert_eq!(list_array.len(), 1);

        // Verify the size
        let inner = list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()
            .expect("Inner should be UInt8Array");

        assert_eq!(inner.len(), 1920 * 1080 * 3);
    }

    #[test]
    fn test_blob_serialization_with_nones() {
        // Exercise the optimized path when null entries are interleaved with data.
        let data = vec![9u8, 8, 7, 6];
        let blob = Blob::from(data.clone());

        let blobs = vec![None, Some(blob)];
        let array = Blob::to_arrow_opt(blobs).unwrap();

        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");

        assert_eq!(list_array.len(), 2);

        let offsets = list_array.offsets();
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 0);
        #[expect(clippy::cast_possible_wrap)]
        let expected_offset = data.len() as i32;
        assert_eq!(offsets[2], expected_offset);

        let validity = list_array.nulls().expect("Null buffer is expected");
        assert!(!validity.is_valid(0));
        assert!(validity.is_valid(1));

        let inner = list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()
            .expect("Inner should be UInt8Array");

        let inner_slice: Vec<u8> = inner.values().iter().copied().collect();
        assert_eq!(inner_slice, data);
    }

    #[test]
    fn test_borrowed_blob_serialization() {
        // Ensure borrowed blobs (Cow::Borrowed path) are handled without copying.
        let data = vec![1u8, 3, 5, 7, 9];
        let blob = Blob::from(data.clone());

        // Serialize using a borrowed reference.
        let array = Blob::to_arrow_opt([Some(&blob)]).unwrap();

        let list_array = array
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .expect("Should be a ListArray");
        assert_eq!(list_array.len(), 1);

        let inner = list_array
            .values()
            .as_any()
            .downcast_ref::<arrow::array::PrimitiveArray<arrow::datatypes::UInt8Type>>()
            .expect("Inner should be UInt8Array");
        assert_eq!(inner.len(), data.len());
        assert_eq!(inner.values().as_ref(), data.as_slice());
    }
}
