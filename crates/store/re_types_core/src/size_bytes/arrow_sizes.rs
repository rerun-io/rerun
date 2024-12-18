use arrow::array::{Array, ArrayRef};

use super::SizeBytes;

impl SizeBytes for dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        Box::<dyn arrow2::array::Array>::from(self).heap_size_bytes()
    }
}

impl SizeBytes for ArrayRef {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        Box::<dyn arrow2::array::Array>::from(self.as_ref()).heap_size_bytes()
    }
}
