use arrow::array::{Array, ArrayRef};

/// Move an arrow array into an [`ArrayRef`].
#[allow(unused)]
pub fn as_array_ref<T: Array + 'static>(t: T) -> ArrayRef {
    std::sync::Arc::new(t) as ArrayRef
}
