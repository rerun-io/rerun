use arrow::datatypes::ArrowNativeType;

/// Convenience-wrapper around an [`arrow2::buffer::Buffer`] that is known to contain a
/// a primitive type.
///
/// The [`ArrowBuffer`] object is internally reference-counted and can be
/// easily converted back to a `&[T]` referencing the underlying storage.
/// This avoids some of the lifetime complexities that would otherwise
/// arise from returning a `&[T]` directly, but is significantly more
/// performant than doing the full allocation necessary to return a `Vec<T>`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArrowBuffer<T: ArrowNativeType>(arrow2::buffer::Buffer<T>);

impl<T: crate::SizeBytes + ArrowNativeType> crate::SizeBytes for ArrowBuffer<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self(buf) = self;
        std::mem::size_of_val(buf.as_slice()) as _
    }
}

impl<T: ArrowNativeType> ArrowBuffer<T> {
    /// The number of instances of T stored in this buffer.
    #[inline]
    pub fn num_instances(&self) -> usize {
        // WARNING: If you are touching this code, make sure you know what len() actually does.
        //
        // There is ambiguity in how arrow2 and arrow-rs talk about buffer lengths, including
        // some incorrect documentation: https://github.com/jorgecarleitao/arrow2/issues/1430
        //
        // Arrow2 `Buffer<T>` is typed and `len()` is the number of units of `T`, but the documentation
        // is currently incorrect.
        // Arrow-rs `Buffer` is untyped and len() is in bytes, but `ScalarBuffer`s are in units of T.
        self.0.len()
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
        self.0.as_slice()
    }

    /// Returns a new [`ArrowBuffer`] that is a slice of this buffer starting at `offset`.
    ///
    /// Doing so allows the same memory region to be shared between buffers.
    ///
    /// # Panics
    /// Panics iff `offset + length` is larger than `len`.
    #[inline]
    pub fn sliced(self, range: std::ops::Range<usize>) -> Self {
        Self(self.0.sliced(range.start, range.len()))
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
        // TODO(emilk): when we switch from arrow2, see if we can make this function zero-copy
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
        self.0.as_slice().to_vec()
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

impl<T: ArrowNativeType> From<arrow2::buffer::Buffer<T>> for ArrowBuffer<T> {
    #[inline]
    fn from(value: arrow2::buffer::Buffer<T>) -> Self {
        Self(value)
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
        Self(arrow2::buffer::Buffer::from_iter(iter))
    }
}

impl<T: ArrowNativeType> std::ops::Deref for ArrowBuffer<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        self.0.as_slice()
    }
}
