use arrow::{
    array::{Array, ArrayRef, ArrowPrimitiveType, PrimitiveArray},
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

impl SizeBytes for ArrayRef {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.get_array_memory_size() as u64
    }
}

impl<T: ArrowPrimitiveType> SizeBytes for PrimitiveArray<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        Array::get_array_memory_size(self) as u64
    }
}

impl<T: ArrowNativeType> SizeBytes for ScalarBuffer<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.inner().capacity() as _
    }
}
