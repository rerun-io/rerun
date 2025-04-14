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
}

impl From<Vec<u8>> for Blob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes.into())
    }
}

impl From<&[u8]> for Blob {
    fn from(bytes: &[u8]) -> Self {
        Self(bytes.into())
    }
}

impl std::ops::Deref for Blob {
    type Target = re_types_core::ArrowBuffer<u8>;

    #[inline]
    fn deref(&self) -> &re_types_core::ArrowBuffer<u8> {
        &self.0
    }
}

impl From<arrow::buffer::ScalarBuffer<u8>> for Blob {
    #[inline]
    fn from(buff: arrow::buffer::ScalarBuffer<u8>) -> Self {
        Self(buff.into())
    }
}
