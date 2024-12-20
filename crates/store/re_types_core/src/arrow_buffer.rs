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
        std::mem::size_of_val(self.as_slice()) as _
    }
}

impl<T: ArrowNativeType> ArrowBuffer<T> {
    /// The number of instances of T stored in this buffer.
    #[inline]
    pub fn num_instances(&self) -> usize {
        self.as_slice().len()
    }

    /// The number of bytes stored in this buffer
    #[inline]
    pub fn size_in_bytes(&self) -> usize {
        self.0.len() * std::mem::size_of::<T>()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.0.as_ref()
    }

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
    /// Cast POD (plain-old-data) types to another POD type.
    ///
    /// For instance: cast a buffer of `u8` to a buffer of `f32`.
    #[inline]
    pub fn cast_pod<Target: bytemuck::Pod + ArrowNativeType>(
        &self,
    ) -> Result<ArrowBuffer<Target>, bytemuck::PodCastError> {
        // TODO(#2978): when we switch from arrow2, see if we can make this function zero-copy
        re_tracing::profile_function!();
        let target_slice: &[Target] = bytemuck::try_cast_slice(self.as_slice())?;
        Ok(ArrowBuffer::from(target_slice.to_vec()))
    }

    /// Cast POD (plain-old-data) types to `u8`.
    #[inline]
    pub fn cast_to_u8(&self) -> ArrowBuffer<u8> {
        match self.cast_pod() {
            Ok(buf) => buf,
            Err(_) => unreachable!("We can always cast POD types to u8"),
        }
    }
}

impl<T: Eq + ArrowNativeType> Eq for ArrowBuffer<T> {}

impl<T: ArrowNativeType> ArrowBuffer<T> {
    #[inline]
    pub fn to_vec(&self) -> Vec<T> {
        self.as_slice().to_vec()
    }
}

impl<T: ArrowNativeType + arrow2::types::NativeType> From<arrow::buffer::ScalarBuffer<T>>
    for ArrowBuffer<T>
{
    #[inline]
    fn from(value: arrow::buffer::ScalarBuffer<T>) -> Self {
        Self(value.into_inner().into())
    }
}

impl<T: ArrowNativeType + arrow2::types::NativeType> From<arrow2::buffer::Buffer<T>>
    for ArrowBuffer<T>
{
    #[inline]
    fn from(arrow2_buffer: arrow2::buffer::Buffer<T>) -> Self {
        let num_elements = arrow2_buffer.len();
        let arrow1_buffer = arrow::buffer::Buffer::from(arrow2_buffer);
        let scalar_buffer = arrow::buffer::ScalarBuffer::new(arrow1_buffer, 0, num_elements);
        Self(scalar_buffer)
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
        self.as_slice().iter()
    }
}

impl<T: ArrowNativeType> std::ops::Deref for ArrowBuffer<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrow2_compatibility() {
        let arrow2_buffer = arrow2::buffer::Buffer::<f32>::from(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(arrow2_buffer.as_slice(), &[1.0, 2.0, 3.0, 4.0, 5.0]);

        let sliced_arrow2_buffer = arrow2_buffer.sliced(1, 3);
        assert_eq!(sliced_arrow2_buffer.as_slice(), &[2.0, 3.0, 4.0]);

        let arrow_buffer = ArrowBuffer::<f32>::from(sliced_arrow2_buffer);
        assert_eq!(arrow_buffer.num_instances(), 3);
        assert_eq!(arrow_buffer.as_slice(), &[2.0, 3.0, 4.0]);
    }
}
