use arrow::datatypes::ArrowNativeType;
use re_byte_size::SizeBytes;

/// Convenience-wrapper around an [`arrow::buffer::ScalarBuffer`] that is known to contain a
/// a primitive type.
///
/// The [`ArrowBuffer`] object is internally reference-counted and can be
/// easily converted back to a `&[T]` referencing the underlying storage.
/// This avoids some of the lifetime complexities that would otherwise
/// arise from returning a `&[T]` directly, but is significantly more
/// performant than doing the full allocation necessary to return a `Vec<T>`.
// TODO(#9725): remove. This is just a `ArrowScalarBuffer` anyway
#[derive(Clone, Debug, PartialEq)]
pub struct ArrowBuffer<T: ArrowNativeType>(arrow::buffer::ScalarBuffer<T>);

impl<T: ArrowNativeType> Default for ArrowBuffer<T> {
    fn default() -> Self {
        Self(arrow::buffer::ScalarBuffer::<T>::from(vec![]))
    }
}

impl<T: SizeBytes + ArrowNativeType> SizeBytes for ArrowBuffer<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let slice: &[T] = self;
        std::mem::size_of_val(slice) as _
    }
}

impl<T: ArrowNativeType> ArrowBuffer<T> {
    /// Returns a new [`ArrowBuffer`] that is a slice of this buffer starting at `offset`.
    ///
    /// Doing so allows the same memory region to be shared between buffers.
    ///
    /// # Panics
    /// Panics iff `offset + length` is larger than `len`.
    #[inline]
    pub fn sliced(self, range: std::ops::Range<usize>) -> Self {
        Self(self.0.slice(range.start, range.len()))
    }
}

impl<T: bytemuck::Pod + ArrowNativeType> ArrowBuffer<T> {
    /// Reinterpret POD (plain-old-data) types to `u8`.
    #[inline]
    pub fn cast_to_u8(&self) -> ArrowBuffer<u8> {
        ArrowBuffer::<u8>(arrow::buffer::ScalarBuffer::new(
            self.inner().clone(),
            0,
            self.inner().len(),
        ))
    }
}

impl<T: Eq + ArrowNativeType> Eq for ArrowBuffer<T> {}

impl<T: ArrowNativeType> From<arrow::buffer::ScalarBuffer<T>> for ArrowBuffer<T> {
    #[inline]
    fn from(value: arrow::buffer::ScalarBuffer<T>) -> Self {
        Self(value.into_inner().into())
    }
}

impl<T: ArrowNativeType> From<Vec<T>> for ArrowBuffer<T> {
    #[inline]
    fn from(value: Vec<T>) -> Self {
        Self(value.into())
    }
}

impl<T: ArrowNativeType> From<&[T]> for ArrowBuffer<T> {
    #[inline]
    fn from(value: &[T]) -> Self {
        Self(value.iter().copied().collect())
    }
}

impl<T: ArrowNativeType> FromIterator<T> for ArrowBuffer<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self(arrow::buffer::ScalarBuffer::from_iter(iter))
    }
}

impl<'a, T: ArrowNativeType> IntoIterator for &'a ArrowBuffer<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T: ArrowNativeType> std::ops::Deref for ArrowBuffer<T> {
    type Target = arrow::buffer::ScalarBuffer<T>;

    #[inline]
    fn deref(&self) -> &arrow::buffer::ScalarBuffer<T> {
        &self.0
    }
}
