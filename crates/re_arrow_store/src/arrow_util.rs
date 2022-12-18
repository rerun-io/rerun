use arrow2::array::Array;

// ---

pub fn is_dense_array(arr: &dyn Array) -> bool {
    arr.validity().is_none()
}
