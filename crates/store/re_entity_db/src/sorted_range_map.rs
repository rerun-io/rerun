use std::ops::RangeInclusive;

/// A sorted, immutable collection of inclusive ranges mapped to values.
///
/// Supports O(log N) queries for overlapping ranges.
#[derive(Debug, Clone, Default)]
pub struct SortedRangeMap<K, V> {
    /// Entries sorted by `range.start()`.
    entries: Vec<(RangeInclusive<K>, V)>,

    /// `max_end[i] = maximum range.end()` for `entries[0..=i]`.
    /// Used to prune the search space.
    max_end: Vec<K>,
}

impl<K, V> re_byte_size::SizeBytes for SortedRangeMap<K, V>
where
    K: re_byte_size::SizeBytes + Ord + Copy,
    V: re_byte_size::SizeBytes,
{
    fn heap_size_bytes(&self) -> u64 {
        let Self { entries, max_end } = self;
        entries.heap_size_bytes() + max_end.heap_size_bytes()
    }
}

impl<K: Ord + Copy, V> SortedRangeMap<K, V> {
    pub fn new(mut entries: Vec<(RangeInclusive<K>, V)>) -> Self {
        entries.sort_by(|a, b| a.0.start().cmp(b.0.start()));

        let mut max_end = Vec::with_capacity(entries.len());
        let mut running_max = None::<K>;

        for (range, _) in &entries {
            let new_max = match running_max {
                Some(m) => m.max(*range.end()),
                None => *range.end(),
            };
            running_max = Some(new_max);
            max_end.push(new_max);
        }

        Self { entries, max_end }
    }

    /// Returns an iterator over all (range, value) pairs that overlap with `query`.
    /// Results are yielded in order of `range.start()` (ascending).
    ///
    /// This is O(log N) to find the starting point, then O(K) for K results.
    #[inline]
    pub fn query(&self, query: RangeInclusive<K>) -> OverlapIter<'_, K, V> {
        let start_idx = self.find_first_possible(&query);

        OverlapIter {
            map: self,
            query,
            idx: start_idx,
        }
    }

    /// Find the first index that could possibly overlap with the query.
    #[inline]
    fn find_first_possible(&self, query: &RangeInclusive<K>) -> usize {
        // We need max_end[i] >= query.start for any overlap to be possible
        self.max_end.partition_point(|max| *max < *query.start())
    }

    #[inline]
    #[cfg_attr(not(test), expect(dead_code))] // only used in tests
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    #[cfg_attr(not(test), expect(dead_code))] // only used in tests
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Non-allocating iterator over overlapping ranges.
#[derive(Debug, Clone)]
pub struct OverlapIter<'a, K, V> {
    map: &'a SortedRangeMap<K, V>,
    query: RangeInclusive<K>,
    idx: usize,
}

impl<'a, K: Ord + Copy, V> Iterator for OverlapIter<'a, K, V> {
    type Item = (&'a RangeInclusive<K>, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.map.entries.len() {
            let (range, value) = &self.map.entries[self.idx];

            if self.query.end() < range.start() {
                // all subsequent ranges start even later due to sorting
                return None;
            }

            self.idx += 1;

            // Check overlap: range.start <= query.end (guaranteed above) && query.start <= range.end
            if self.query.start() <= range.end() {
                return Some((range, value));
            }
            // Otherwise this range ends before our query starts; skip it
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.map.entries.len().saturating_sub(self.idx)))
    }
}

impl<K: Ord + Copy, V> std::iter::FusedIterator for OverlapIter<'_, K, V> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_overlap() {
        let map = SortedRangeMap::new(vec![
            (0..=10, "a"),
            (5..=15, "b"),
            (20..=30, "c"),
            (25..=35, "d"),
        ]);

        // Query overlapping first two
        let results: Vec<_> = map.query(7..=12).collect();
        assert_eq!(results, vec![(&(0..=10), &"a"), (&(5..=15), &"b")]);

        // Query overlapping none (gap between b and c)
        let results: Vec<_> = map.query(16..=19).collect();
        assert!(results.is_empty());

        // Query overlapping last two
        let results: Vec<_> = map.query(27..=32).collect();
        assert_eq!(results, vec![(&(20..=30), &"c"), (&(25..=35), &"d")]);
    }

    #[test]
    fn test_inclusive_boundaries() {
        let map = SortedRangeMap::new(vec![(10..=20, "x")]);

        // Touching at boundaries IS overlapping for inclusive ranges
        let results: Vec<_> = map.query(0..=10).collect();
        assert_eq!(results, vec![(&(10..=20), &"x")]);

        let results: Vec<_> = map.query(20..=30).collect();
        assert_eq!(results, vec![(&(10..=20), &"x")]);

        // Just outside
        assert!(map.query(0..=9).next().is_none());
        assert!(map.query(21..=30).next().is_none());
    }

    #[test]
    fn test_point_queries() {
        let map = SortedRangeMap::new(vec![(0..=10, "a"), (10..=20, "b"), (20..=30, "c")]);

        // Point query at shared boundary
        let results: Vec<_> = map.query(10..=10).collect();
        assert_eq!(results, vec![(&(0..=10), &"a"), (&(10..=20), &"b")]);

        // Point query at another shared boundary
        let results: Vec<_> = map.query(20..=20).collect();
        assert_eq!(results, vec![(&(10..=20), &"b"), (&(20..=30), &"c")]);

        // Point query in middle of range
        let results: Vec<_> = map.query(5..=5).collect();
        assert_eq!(results, vec![(&(0..=10), &"a")]);
    }

    #[test]
    fn test_fully_contained() {
        let map = SortedRangeMap::new(vec![
            (0..=100, "outer"),
            (20..=30, "inner1"),
            (40..=50, "inner2"),
        ]);

        // Query that hits all three
        let results: Vec<_> = map.query(25..=45).collect();
        assert_eq!(
            results,
            vec![
                (&(0..=100), &"outer"),
                (&(20..=30), &"inner1"),
                (&(40..=50), &"inner2"),
            ]
        );

        // Query fully inside outer but missing inners
        let results: Vec<_> = map.query(31..=39).collect();
        assert_eq!(results, vec![(&(0..=100), &"outer")]);
    }

    #[test]
    fn test_empty_map() {
        let map: SortedRangeMap<i32, ()> = SortedRangeMap::new(vec![]);
        assert!(map.query(0..=100).next().is_none());
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_single_element() {
        let map = SortedRangeMap::new(vec![(50..=60, "only")]);

        assert!(map.query(0..=49).next().is_none());
        assert!(map.query(61..=100).next().is_none());
        assert_eq!(map.query(50..=60).count(), 1);
        assert_eq!(map.query(55..=55).count(), 1);
    }

    #[test]
    fn test_many_overlapping() {
        // Ranges that all overlap each other
        let map = SortedRangeMap::new(vec![
            (0..=10, 0),
            (1..=11, 1),
            (2..=12, 2),
            (3..=13, 3),
            (4..=14, 4),
        ]);

        // Query that hits all
        let results: Vec<_> = map.query(5..=5).collect();
        assert_eq!(results.len(), 5);

        // Verify order is by start
        let starts: Vec<_> = results.iter().map(|(r, _)| *r.start()).collect();
        assert_eq!(starts, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_disjoint_ranges() {
        let map = SortedRangeMap::new(vec![
            (0..=10, "a"),
            (20..=30, "b"),
            (40..=50, "c"),
            (60..=70, "d"),
            (80..=90, "e"),
        ]);

        // Query in gaps
        assert!(map.query(11..=19).next().is_none());
        assert!(map.query(31..=39).next().is_none());
        assert!(map.query(51..=59).next().is_none());
        assert!(map.query(71..=79).next().is_none());

        // Query spanning multiple with gaps
        let results: Vec<_> = map.query(25..=65).collect();
        assert_eq!(
            results,
            vec![(&(20..=30), &"b"), (&(40..=50), &"c"), (&(60..=70), &"d")]
        );
    }

    #[test]
    fn test_query_larger_than_all() {
        let map = SortedRangeMap::new(vec![(10..=20, "a"), (30..=40, "b"), (50..=60, "c")]);

        let results: Vec<_> = map.query(0..=100).collect();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_unsorted_input() {
        // Input not sorted - should still work
        let map = SortedRangeMap::new(vec![(50..=60, "c"), (10..=20, "a"), (30..=40, "b")]);

        let results: Vec<_> = map.query(0..=100).collect();
        // Should be sorted by start in output
        let values: Vec<_> = results.iter().map(|(_, v)| **v).collect();
        assert_eq!(values, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_duplicate_starts() {
        let map = SortedRangeMap::new(vec![(10..=20, "a"), (10..=30, "b"), (10..=15, "c")]);

        let results: Vec<_> = map.query(12..=12).collect();
        assert_eq!(results.len(), 3);
    }
}
