use smallvec::SmallVec;

use crate::SizeBytes;

impl<T: SizeBytes, const N: usize> SizeBytes for SmallVec<[T; N]> {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        if self.len() <= N {
            // The `SmallVec` is still smaller than the threshold so no heap data has been
            // allocated yet, beyond the heap data each element might have.

            if T::is_pod() {
                0 // early-out
            } else {
                self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
            }
        } else {
            // NOTE: It's all on the heap at this point.
            if T::is_pod() {
                (self.len() * std::mem::size_of::<T>()) as _
            } else {
                self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
            }
        }
    }
}
