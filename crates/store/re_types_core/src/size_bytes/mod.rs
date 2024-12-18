mod arrow2_sizes;
mod arrow_sizes;

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;

use smallvec::SmallVec;

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

// TODO(rust-lang/rust#31844): This isn't happening without specialization.
// impl<T> SizeBytes for T where T: bytemuck::Pod { … }

// --- Std ---

impl SizeBytes for String {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.as_bytes().len() as u64
    }
}

impl<K: SizeBytes, V: SizeBytes> SizeBytes for BTreeMap<K, V> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.

        let keys_size_bytes = if K::is_pod() {
            (self.len() * std::mem::size_of::<K>()) as _
        } else {
            self.keys().map(SizeBytes::total_size_bytes).sum::<u64>()
        };

        let values_size_bytes = if V::is_pod() {
            (self.len() * std::mem::size_of::<V>()) as _
        } else {
            self.values().map(SizeBytes::total_size_bytes).sum::<u64>()
        };

        keys_size_bytes + values_size_bytes
    }
}

impl<K: SizeBytes> SizeBytes for BTreeSet<K> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.

        if K::is_pod() {
            (self.len() * std::mem::size_of::<K>()) as _
        } else {
            self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
        }
    }
}

impl<K: SizeBytes, V: SizeBytes, S> SizeBytes for HashMap<K, V, S> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.

        let keys_size_bytes = if K::is_pod() {
            (self.len() * std::mem::size_of::<K>()) as _
        } else {
            self.keys().map(SizeBytes::total_size_bytes).sum::<u64>()
        };

        let values_size_bytes = if V::is_pod() {
            (self.len() * std::mem::size_of::<V>()) as _
        } else {
            self.values().map(SizeBytes::total_size_bytes).sum::<u64>()
        };

        keys_size_bytes + values_size_bytes
    }
}

// NOTE: Do _not_ implement `SizeBytes` for slices: we cannot know whether they point to the stack
// or the heap!

impl<T: SizeBytes, const N: usize> SizeBytes for [T; N] {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        if T::is_pod() {
            0 // it's a const-sized array
        } else {
            self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
        }
    }
}

impl<T: SizeBytes> SizeBytes for Vec<T> {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        if T::is_pod() {
            (self.len() * std::mem::size_of::<T>()) as _
        } else {
            self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
        }
    }
}

impl<T: SizeBytes> SizeBytes for VecDeque<T> {
    /// Does not take capacity into account.
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        if T::is_pod() {
            (self.len() * std::mem::size_of::<T>()) as _
        } else {
            self.iter().map(SizeBytes::total_size_bytes).sum::<u64>()
        }
    }
}

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

impl<T: SizeBytes> SizeBytes for Option<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.as_ref().map_or(0, SizeBytes::heap_size_bytes)
    }
}

impl<T: SizeBytes> SizeBytes for Arc<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0 // assume it's amortized
    }
}

impl<T: SizeBytes> SizeBytes for Box<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        T::total_size_bytes(&**self)
    }
}

// TODO(rust-lang/rust#31844): `impl<T: bytemuck::Pod> SizeBytesExt for T {}` would be nice but
// violates orphan rules.
macro_rules! impl_size_bytes_pod {
    ($ty:ty) => {
        impl SizeBytes for $ty {
            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }

            #[inline]
            fn is_pod() -> bool {
                true
            }
        }
    };
    ($ty:ty, $($rest:ty),+) => {
        impl_size_bytes_pod!($ty); impl_size_bytes_pod!($($rest),+);
    };
}

impl_size_bytes_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, f32, f64);
impl_size_bytes_pod!(half::f16);

impl<T, U> SizeBytes for (T, U)
where
    T: SizeBytes,
    U: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b) = self;
        a.heap_size_bytes() + b.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod()
    }
}

impl<T, U, V> SizeBytes for (T, U, V)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod() && V::is_pod()
    }
}

impl<T, U, V, W> SizeBytes for (T, U, V, W)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
    W: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c, d) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes() + d.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod() && V::is_pod() && W::is_pod()
    }
}
