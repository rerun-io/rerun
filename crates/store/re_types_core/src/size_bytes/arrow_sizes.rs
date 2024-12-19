use arrow::array::{Array, ArrayRef};

use super::SizeBytes;

impl SizeBytes for dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.get_array_memory_size() as u64
    }
}

impl SizeBytes for ArrayRef {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.get_array_memory_size() as u64
    }
}
