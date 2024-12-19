//! Calculate the heap-allocated size of values at runtime.

mod arrow2_sizes;
mod arrow_sizes;
mod primitive_sizes;
mod smallvec_sizes;
mod std_sizes;
mod tuple_sizes;

// ---

/// Approximations of stack and heap size for both internal and external types.
///
/// Motly used for statistics and triggering events such as garbage collection.
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

    /// Returns the total size of `self` on the heap, in bytes.
    fn heap_size_bytes(&self) -> u64;

    /// Is `Self` just plain old data?
    ///
    /// If `true`, this will make most blanket implementations of `SizeBytes` much faster (e.g. `Vec<T>`).
    #[inline]
    fn is_pod() -> bool {
        false
    }
}

impl SizeBytes for re_tuid::Tuid {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}
