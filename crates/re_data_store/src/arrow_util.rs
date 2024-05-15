use arrow2::array::{Array, ListArray};

// ---

pub trait ArrayExt: Array {
    /// Returns the length of the child array at the given index.
    ///
    /// * Panics if `self` is not a `ListArray<i32>`.
    /// * Panics if `child_nr` is out of bounds.
    fn get_child_length(&self, child_nr: usize) -> usize;
}

impl ArrayExt for dyn Array {
    /// Return the length of the first child.
    ///
    /// ## Panics
    ///
    /// Panics if `Self` is not a `ListArray<i32>`, or if the array is empty (no children).
    fn get_child_length(&self, child_nr: usize) -> usize {
        self.as_any()
            .downcast_ref::<ListArray<i32>>()
            .expect("not a ListArray<i32>")
            .offsets()
            .lengths()
            .nth(child_nr)
            .unwrap_or_else(|| panic!("no child at index {child_nr}"))
    }
}
