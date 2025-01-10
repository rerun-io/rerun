use arrow::{
    array::{Array, ArrayRef},
    buffer::ScalarBuffer,
    datatypes::ArrowNativeType,
};

use super::SizeBytes;

impl SizeBytes for dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.get_array_memory_size() as u64
    }
}

impl SizeBytes for &dyn Array {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.get_array_memory_size() as u64
    }
}

impl<T: Array> SizeBytes for &T {
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

impl<T: ArrowNativeType> SizeBytes for ScalarBuffer<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.inner().capacity() as _
    }
}
