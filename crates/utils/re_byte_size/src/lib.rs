//! Calculate the heap-allocated size of values at runtime.

mod arrow_sizes;
mod bookkeeping_btreemap;
mod primitive_sizes;
mod smallvec_sizes;
mod std_sizes;
mod tuple_sizes;

pub use bookkeeping_btreemap::BookkeepingBTreeMap;

// ---

/// Approximations of stack and heap size for both internal and external types.
///
/// Motly used for statistics and triggering events such as garbage collection.
// TODO(#8630): Derive macro for this trait.
pub trait SizeBytes {
    /// Returns the total size of `self` in bytes, accounting for both stack and heap space.
    #[inline]
    fn total_size_bytes(&self) -> u64 {
        self.stack_size_bytes() + self.heap_size_bytes()
    }

    /// Returns the total size of `self` on the stack, in bytes.
    ///
    /// Defaults to `std::mem::size_of_val(self)`.
    #[inline]
    fn stack_size_bytes(&self) -> u64 {
        std::mem::size_of_val(self) as _
    }

    /// Returns how many bytes `self` uses on the heap.
    ///
    /// In some cases `self` may be just a slice of a larger buffer.
    /// This will in that case only return the memory used by that smaller slice.
    ///
    /// If we however are the sole owner of the memory (e.g. a `Vec`), then we return
    /// the heap size of all children plus the capacity of the buffer.
    fn heap_size_bytes(&self) -> u64;

    /// Is `Self` just plain old data?
    ///
    /// If `true`, this will make most blanket implementations of `SizeBytes` much faster (e.g. `Vec<T>`).
    #[inline]
    fn is_pod() -> bool {
        false
    }
}
