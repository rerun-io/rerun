//! Implement [`SizeBytes`] for things in the standard library.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::mem::size_of;
use std::ops::RangeInclusive;
use std::sync::Arc;

use crate::SizeBytes;

impl SizeBytes for String {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.capacity() as u64
    }
}

// ----------------------------------------------------------------------------
// BTree collections

/// Estimate heap size for a [`BTreeMap`] or [`BTreeSet`],
/// excluding the memory that non-POD key/values hold on their own.
#[inline]
pub(crate) fn btree_heap_size(len: usize, entry_size: usize) -> u64 {
    if len == 0 {
        return 0;
    }

    // Reference: https://github.com/rust-lang/rust/blob/main/library/alloc/src/collections/btree/node.rs

    const BTREE_B: usize = 6;
    const ELEMENTS_PER_LEAF: usize = 2 * BTREE_B - 1;
    const NODE_FIXED_OVERHEAD: usize = 16;

    // Estimate number of leaf nodes needed (nodes are ~half full on average after insertions)
    let num_leaf_nodes = len.div_ceil(ELEMENTS_PER_LEAF);

    // Each leaf node allocates space for ELEMENTS_PER_LEAF entries
    let leaf_data_size = num_leaf_nodes * ELEMENTS_PER_LEAF * entry_size;
    let leaf_overhead = num_leaf_nodes * NODE_FIXED_OVERHEAD;

    // Internal nodes add ~10% overhead for large trees, ignore for simplicity
    (leaf_data_size + leaf_overhead) as u64
}

impl<K: SizeBytes, V: SizeBytes> SizeBytes for BTreeMap<K, V> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        // BTree stores keys and values in separate arrays within nodes,
        // so there's no tuple padding like in HashMap.
        let base_size = btree_heap_size(self.len(), size_of::<K>() + size_of::<V>());

        let heap_in_keys = if K::is_pod() {
            0
        } else {
            self.keys().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        let heap_in_values = if V::is_pod() {
            0
        } else {
            self.values().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        base_size + heap_in_keys + heap_in_values
    }
}

impl<K: SizeBytes> SizeBytes for BTreeSet<K> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        let base_size = btree_heap_size(self.len(), size_of::<K>());

        let heap_in_keys = if K::is_pod() {
            0
        } else {
            self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        base_size + heap_in_keys
    }
}

// ----------------------------------------------------------------------------
// Hash collections

/// Estimate the number of slots allocated by a hashmap for the given capacity.
///
/// stdlib uses `hashbrown` hashmaps.
///
/// Reference: <https://github.com/rust-lang/hashbrown/blob/9037471eb241119de665eb328030f4b19c63dcbe/src/raw/mod.rs#L190-L191>
#[inline]
fn hashbrown_num_slots(capacity: usize) -> usize {
    if capacity == 0 {
        0
    } else if capacity < 4 {
        4
    } else if capacity < 8 {
        8
    } else {
        // hashbrown maintains 87.5% load factor: buckets = capacity * 8 / 7
        (capacity * 8).div_ceil(7)
    }
}

impl<K: SizeBytes, V: SizeBytes, S> SizeBytes for HashMap<K, V, S> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        let num_slots = hashbrown_num_slots(self.capacity());

        // 1 control byte per slot for SIMD metadata
        let control_bytes = num_slots as u64;

        // Use size_of::<(K, V)>() to account for alignment padding between K and V.
        // For example, (u32, u8) takes 8 bytes, not 5.
        let entry_size = (num_slots * size_of::<(K, V)>()) as u64;

        let heap_in_keys = if K::is_pod() {
            0
        } else {
            self.keys().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        let heap_in_values = if V::is_pod() {
            0
        } else {
            self.values().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        control_bytes + entry_size + heap_in_keys + heap_in_values
    }
}

impl<K: SizeBytes, S> SizeBytes for HashSet<K, S> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        let num_slots = hashbrown_num_slots(self.capacity());

        // 1 control byte per slot for SIMD metadata
        let control_bytes = num_slots as u64;

        let entry_size = (num_slots * size_of::<K>()) as u64;

        let heap_in_keys = if K::is_pod() {
            0
        } else {
            self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
        };

        control_bytes + entry_size + heap_in_keys
    }
}

// ----------------------------------------------------------------------------

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
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        if T::is_pod() {
            (self.capacity() * size_of::<T>()) as _
        } else {
            (self.capacity() * size_of::<T>()) as u64
                + self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
        }
    }
}

impl<T: SizeBytes> SizeBytes for VecDeque<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // NOTE: It's all on the heap at this point.
        if T::is_pod() {
            (self.capacity() * size_of::<T>()) as _
        } else {
            (self.capacity() * size_of::<T>()) as u64
                + self.iter().map(SizeBytes::heap_size_bytes).sum::<u64>()
        }
    }
}

impl<T: SizeBytes> SizeBytes for Option<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.as_ref().map_or(0, SizeBytes::heap_size_bytes)
    }
}

impl<T: SizeBytes, E: SizeBytes> SizeBytes for Result<T, E> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Ok(value) => value.heap_size_bytes(),
            Err(err) => err.heap_size_bytes(),
        }
    }
}

impl<T: SizeBytes> SizeBytes for Arc<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // Overhead for strong and weak counts:
        let arc_overhead = 2 * size_of::<usize>() as u64;

        // A good approximation, that crucially works well for strong_count=1:
        (T::total_size_bytes(&**self) + arc_overhead) / Self::strong_count(self) as u64
    }
}

impl<T: SizeBytes> SizeBytes for Box<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        T::total_size_bytes(&**self)
    }
}

impl<T: SizeBytes> SizeBytes for RangeInclusive<T> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.start().heap_size_bytes() + self.end().heap_size_bytes()
    }
}
