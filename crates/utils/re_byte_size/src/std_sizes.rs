//! Implement [`SizeBytes`] for things in the standard library.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::Arc,
};

use crate::SizeBytes;

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
