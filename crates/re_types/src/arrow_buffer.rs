use arrow2::buffer::Buffer;

/// Convenience-wrapper around an arrow [`Buffer`] that is known to contain a
/// a primitive type.
///
/// The arrow2 [`Buffer`] object is internally reference-counted and can be
/// easily converted back to a `&[T]` referencing the underlying storage.
/// This avoids some of the lifetime complexities that would otherwise
/// arise from returning a `&[T]` directly, but is significantly more
/// performant than doing the full allocation necessary to return a `Vec<T>`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArrowBuffer<T>(pub Buffer<T>);

impl<T> ArrowBuffer<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> From<Vec<T>> for ArrowBuffer<T> {
    fn from(value: Vec<T>) -> Self {
        Self(value.into())
    }
}
