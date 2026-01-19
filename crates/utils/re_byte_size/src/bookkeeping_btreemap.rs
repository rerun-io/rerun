//! A `BTreeMap` wrapper with continuous size bookkeeping for instant `SizeBytes` queries.

use std::collections::BTreeMap;

use crate::SizeBytes;

/// A [`BTreeMap`] wrapper with O(1) size queries via continuous bookkeeping.
///
/// Tracks the total size of keys and values, making [`SizeBytes::heap_size_bytes`] instant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookkeepingBTreeMap<K, V> {
    map: BTreeMap<K, V>,

    /// The total heap size of all keys and values in bytes.
    heap_size_bytes: u64,
}

impl<K, V> Default for BookkeepingBTreeMap<K, V>
where
    K: Ord + SizeBytes,
    V: SizeBytes,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> BookkeepingBTreeMap<K, V>
where
    K: Ord + SizeBytes,
    V: SizeBytes,
{
    /// Creates an empty `BookkeepingBTreeMap`.
    #[inline]
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            heap_size_bytes: 0,
        }
    }

    /// Returns `true` if the map contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns an iterator over the key-value pairs of the map, in sorted order by key.
    #[inline]
    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.map.iter()
    }

    /// Mutates an entry, inserting `default_value` if the key doesn't exist.
    ///
    /// Size changes are tracked automatically.
    pub fn mutate_entry(&mut self, key: K, default_value: V, mut mutator: impl FnMut(&mut V)) {
        use std::collections::btree_map::Entry;

        match self.map.entry(key) {
            Entry::Vacant(vacant) => {
                let key_size = vacant.key().total_size_bytes();
                let value_ref = vacant.insert(default_value);
                mutator(value_ref);
                let value_size = value_ref.total_size_bytes();
                self.heap_size_bytes += key_size + value_size;
            }
            Entry::Occupied(mut occupied) => {
                let size_before = occupied.get().total_size_bytes();
                mutator(occupied.get_mut());
                let size_after = occupied.get().total_size_bytes();
                self.heap_size_bytes = self.heap_size_bytes - size_before + size_after;
            }
        }
    }

    /// Finds and mutates the last entry smaller or equal to the given `key`.
    ///
    /// Equivalent to `.range_mut(..=key).next_back()` but with automatic size tracking.
    /// Returns the mutator's return value, or `None` if no entry exists <= `key`.
    pub fn mutate_latest_at<Ret>(
        &mut self,
        key: &K,
        mut mutator: impl FnMut(&K, &mut V) -> Ret,
    ) -> Option<Ret>
    where
        K: Clone,
    {
        let (key, value) = self.map.range_mut(..=key).next_back()?;
        let size_before = value.total_size_bytes();
        let ret = mutator(key, value);
        let size_after = value.total_size_bytes();
        self.heap_size_bytes = self.heap_size_bytes - size_before + size_after;
        Some(ret)
    }

    /// Returns a reference to the inner [`BTreeMap`].
    #[inline]
    pub fn as_map(&self) -> &BTreeMap<K, V> {
        &self.map
    }

    /// Inserts a key-value pair, returning the old value if the key was present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // In a BTreeMap, the keys and values themselves are stored on the heap,
        // so we count their total size (stack + heap).
        let new_key_size = key.total_size_bytes();
        let new_value_size = value.total_size_bytes();

        let old_value = self.map.insert(key, value);

        if let Some(old_value) = &old_value {
            // We're replacing an existing value, but the key remains the same:
            self.heap_size_bytes += new_value_size;
            self.heap_size_bytes -= old_value.total_size_bytes();
        } else {
            // New key-value pair - add both sizes:
            self.heap_size_bytes += new_key_size + new_value_size;
        }

        old_value
    }

    /// Removes a key, returning its value if it was present.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.map.remove(key) {
            let key_size = key.total_size_bytes();
            let value_size = value.total_size_bytes();
            self.heap_size_bytes = self.heap_size_bytes - key_size - value_size;
            Some(value)
        } else {
            None
        }
    }

    /// Extends the map with key-value pairs from an iterator.
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (K, V)>,
    {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<K: SizeBytes, V: SizeBytes> SizeBytes for BookkeepingBTreeMap<K, V> {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        // O(1) - this is the whole point!
        self.heap_size_bytes
    }
}

impl<'a, K, V> IntoIterator for &'a BookkeepingBTreeMap<K, V>
where
    K: Ord + SizeBytes,
    V: SizeBytes,
{
    type Item = (&'a K, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heap_size_of_map<K: Ord + SizeBytes, V: SizeBytes>(map: &BookkeepingBTreeMap<K, V>) -> u64 {
        map.iter()
            .map(|(k, v)| k.total_size_bytes() + v.total_size_bytes())
            .sum()
    }

    #[test]
    fn test_multiple_entries() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();
        assert_eq!(map.heap_size_bytes(), 0);

        map.insert(1, "one".to_owned());
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.insert(2, "two".to_owned());
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.insert(2, "two, but now it is different".to_owned());
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.remove(&1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.remove(&2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
        assert_eq!(map.heap_size_bytes(), 0);
    }

    #[test]
    fn test_new_and_default() {
        let map1: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();
        let map2: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::default();

        assert_eq!(map1.heap_size_bytes(), 0);
        assert_eq!(map2.heap_size_bytes(), 0);
        assert!(map1.is_empty());
        assert!(map2.is_empty());
        assert_eq!(map1.len(), 0);
        assert_eq!(map2.len(), 0);
    }

    #[test]
    fn test_insert_bookkeeping() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        let old = map.insert(1, "hello".to_owned());
        assert_eq!(old, None);
        assert_eq!(map.len(), 1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let old = map.insert(2, "world".to_owned());
        assert_eq!(old, None);
        assert_eq!(map.len(), 2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let old = map.insert(1, "hello, this is much longer!".to_owned());
        assert_eq!(old, Some("hello".to_owned()));
        assert_eq!(map.len(), 2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let old = map.insert(1, "hi".to_owned());
        assert_eq!(old, Some("hello, this is much longer!".to_owned()));
        assert_eq!(map.len(), 2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
    }

    #[test]
    fn test_remove_bookkeeping() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map.insert(1, "one".to_owned());
        map.insert(2, "two".to_owned());
        map.insert(3, "three".to_owned());
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let removed = map.remove(&2);
        assert_eq!(removed, Some("two".to_owned()));
        assert_eq!(map.len(), 2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let removed = map.remove(&99);
        assert_eq!(removed, None);
        assert_eq!(map.len(), 2);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.remove(&1);
        map.remove(&3);
        assert_eq!(map.heap_size_bytes(), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn test_extend_bookkeeping() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map.extend(vec![
            (1, "one".to_owned()),
            (2, "two".to_owned()),
            (3, "three".to_owned()),
        ]);
        assert_eq!(map.len(), 3);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.extend(vec![(2, "TWO".to_owned()), (4, "four".to_owned())]);
        assert_eq!(map.len(), 4);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
    }

    #[test]
    fn test_mutate_entry_bookkeeping() {
        let mut map: BookkeepingBTreeMap<u64, Vec<String>> = BookkeepingBTreeMap::new();

        map.mutate_entry(1, Vec::new(), |vec| {
            vec.push("hello".to_owned());
        });
        assert_eq!(map.len(), 1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.mutate_entry(1, Vec::new(), |vec| {
            vec.push("world".to_owned());
        });
        assert_eq!(map.len(), 1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.mutate_entry(1, Vec::new(), |vec| {
            vec.pop();
        });
        assert_eq!(map.len(), 1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        map.mutate_entry(1, Vec::new(), |vec| {
            vec.clear();
        });
        assert_eq!(map.len(), 1);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
    }

    #[test]
    fn test_mutate_entry_before_bookkeeping() {
        let mut map: BookkeepingBTreeMap<u64, Vec<String>> = BookkeepingBTreeMap::new();

        map.insert(10, vec!["ten".to_owned()]);
        map.insert(20, vec!["twenty".to_owned()]);
        map.insert(30, vec!["thirty".to_owned()]);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let result = map.mutate_latest_at(&20, |key, vec| {
            assert_eq!(*key, 20);
            vec.push("added".to_owned());
            *key
        });
        assert_eq!(result, Some(20));
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let result = map.mutate_latest_at(&100, |key, vec| {
            assert_eq!(*key, 30);
            vec.clear();
            *key
        });
        assert_eq!(result, Some(30));
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));

        let result = map.mutate_latest_at(&5, |_key, vec| {
            vec.push("should not happen".to_owned());
        });
        assert_eq!(result, None);
        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
    }

    #[test]
    fn test_iter() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map.insert(3, "three".to_owned());
        map.insert(1, "one".to_owned());
        map.insert(2, "two".to_owned());

        let items: Vec<_> = map.iter().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(*items[0].0, 1);
        assert_eq!(*items[1].0, 2);
        assert_eq!(*items[2].0, 3);
    }

    #[test]
    fn test_into_iterator() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map.insert(2, "two".to_owned());
        map.insert(1, "one".to_owned());

        let items: Vec<_> = (&map).into_iter().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(*items[0].0, 1);
        assert_eq!(*items[1].0, 2);
    }

    #[test]
    fn test_clone() {
        let mut map1: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map1.insert(1, "one".to_owned());
        map1.insert(2, "two".to_owned());

        let map2 = map1.clone();

        assert_eq!(map1.len(), map2.len());
        assert_eq!(map1.heap_size_bytes(), map2.heap_size_bytes());
        assert_eq!(map1, map2);
        assert_eq!(map2.heap_size_bytes(), heap_size_of_map(&map2));
    }

    #[test]
    fn test_partial_eq() {
        let mut map1: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();
        let mut map2: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map1.insert(1, "one".to_owned());
        map2.insert(1, "one".to_owned());
        assert_eq!(map1, map2);

        map1.insert(2, "two".to_owned());
        assert_ne!(map1, map2);

        map2.insert(2, "TWO".to_owned());
        assert_ne!(map1, map2);

        map2.insert(2, "two".to_owned());
        assert_eq!(map1, map2);
    }

    #[test]
    fn test_as_map() {
        let mut map: BookkeepingBTreeMap<u64, String> = BookkeepingBTreeMap::new();

        map.insert(1, "one".to_owned());
        map.insert(2, "two".to_owned());

        let inner_map = map.as_map();
        assert_eq!(inner_map.len(), 2);
        assert_eq!(inner_map.get(&1), Some(&"one".to_owned()));
        assert_eq!(inner_map.get(&2), Some(&"two".to_owned()));
    }

    #[test]
    fn test_bookkeeping_stress() {
        let mut map: BookkeepingBTreeMap<u64, Vec<String>> = BookkeepingBTreeMap::new();

        for i in 0..100 {
            map.insert(i, vec![format!("value_{i}")]);
            assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
        }

        for i in (0..100).step_by(5) {
            map.mutate_entry(i, Vec::new(), |vec| {
                vec.push(format!("extra_{i}"));
            });
            assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
        }

        for i in (0..100).step_by(3) {
            map.remove(&i);
            assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
        }

        assert_eq!(map.heap_size_bytes(), heap_size_of_map(&map));
    }
}
