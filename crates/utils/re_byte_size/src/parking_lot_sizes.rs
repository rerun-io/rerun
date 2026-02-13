use parking_lot::{Mutex, RwLock};

use crate::SizeBytes;

impl<T> SizeBytes for Mutex<T>
where
    T: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.lock().heap_size_bytes()
    }
}

impl<T> SizeBytes for RwLock<T>
where
    T: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.read().heap_size_bytes()
    }
}
