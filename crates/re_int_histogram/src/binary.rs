// key: i64
// count: u64,

// ----------------------------------------------------------------------------

type Level = u64;
const ROOT_LEVEL: Level = 63;
const LEAF_LEVEL: Level = 0;
const LEVEL_STEP: u64 = 1;
// const CHILDREN_PER_LEVEL: u64 = 16;

// level 0, 4, 8, 16, …, 56, 60
// (1 << level) is the size of the range the next child

#[inline(always)]
fn split_address(level: Level, addr: u64) -> (u64, u64) {
    let top = (addr >> level) & 0b1;
    let bottom = addr & ((1 << level) - 1);
    (top, bottom)
}

/// We use 64-bit keys in the internal structures, because it is so much easier
/// to deal with
/// ``` ignore # TODO: run private function in doctest
/// use crate::u64_key_from_i64_key;
/// debug_assert_eq!(u64_key_from_i64_key(i64::MIN), u64::MIN);
/// debug_assert_eq!(u64_key_from_i64_key(i64::MIN + 1), u64::MIN + 1);
/// debug_assert_eq!(u64_key_from_i64_key(i64::MIN + 2), u64::MIN + 2);
/// debug_assert_eq!(u64_key_from_i64_key(i64::MAX - 2), u64::MAX - 2);
/// debug_assert_eq!(u64_key_from_i64_key(i64::MAX - 1), u64::MAX - 1);
/// debug_assert_eq!(u64_key_from_i64_key(i64::MAX), u64::MAX);
/// ```
fn u64_key_from_i64_key(key: i64) -> u64 {
    (key as i128 + i64::MAX as i128 + 1) as _
    // key as _ // sometimes easier to bug
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub num_nodes: u64,
    pub node_child_count: u64,
    pub node_bytes: u64,

    pub num_sparse: u64,
    pub num_sparse_children: u64,
    pub sparse_bytes: u64,
    pub sparse_bytes_vec: u64,

    pub num_dense: u64,
}

// ----------------------------------------------------------------------------
// High-level API

/// A histogram, mapping [`i64`] key to a [`u64`] count
/// optimizing for very fast range-queries.
#[derive(Clone, Debug)]
pub struct IntHistogram {
    root: Tree,
}

impl Default for IntHistogram {
    fn default() -> Self {
        Self {
            root: Tree::Sparse(Sparse::default()),
        }
    }
}

impl IntHistogram {
    /// Insert in multi-set.
    ///
    /// Increments the count of the given bucket.
    pub fn increment(&mut self, key: i64, inc: u64) {
        self.root
            .increment(ROOT_LEVEL, u64_key_from_i64_key(key), inc);
    }

    /// Total count in all the buckets.
    ///
    /// NOTE: this is not the number of unique keys, but cardinality of the multiset.
    pub fn total_count(&self) -> u64 {
        self.root.total_count()
    }

    /// How many keys in the given range.
    pub fn range_count(&self, range: impl std::ops::RangeBounds<i64>) -> u64 {
        let min = match range.start_bound() {
            std::ops::Bound::Included(min) => *min,
            std::ops::Bound::Excluded(min) => min.saturating_add(1),
            std::ops::Bound::Unbounded => i64::MIN,
        };
        let max = match range.end_bound() {
            std::ops::Bound::Included(min) => *min,
            std::ops::Bound::Excluded(min) => min.saturating_sub(1),
            std::ops::Bound::Unbounded => i64::MAX,
        };
        if max < min {
            return 0;
        }
        self.root.range_count(
            ROOT_LEVEL,
            Range {
                min: u64_key_from_i64_key(min),
                max: u64_key_from_i64_key(max),
            },
        )
    }

    // fn remove_all_in_range(&mut self, min: i64, max: i64)

    pub fn stats(&self) -> Stats {
        let mut stats = Stats::default();
        self.root.collect_stats(&mut stats);
        stats
    }
}

// ----------------------------------------------------------------------------
// Low-level data structure.
// All internal addresses are relative.

#[derive(Clone, Copy, Debug)]
struct Range {
    /// inclusive
    pub min: u64,

    /// inclusive
    pub max: u64,
}

impl Range {
    #[inline]
    pub fn contains(&self, value: u64) -> bool {
        self.min <= value && value <= self.max
    }
}

/// 136 bytes large
/// Has sixteen levels. The root is level 15, the leaves level 0.
#[derive(Clone, Debug)]
enum Tree {
    Node(Node),
    Sparse(Sparse),
    Dense(Dense),
}
static_assertions::assert_eq_size!(Tree, [u64; 4]);

#[derive(Clone, Debug, Default)]
struct Node {
    /// Very important optimization
    total_count: u64,

    /// The index is the next bit of the address
    children: [Option<Box<Tree>>; 2],
}
// static_assertions::assert_eq_size!(Node, [u8; 136]);

#[derive(Clone, Debug, Default)]
struct Sparse {
    /// Sorted (addr, count) pairs
    addr_counts: Vec<(u64, u64)>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Dense {
    /// The last 2 bits of the address, mapped to their counts
    counts: [u32; 4],
}
// static_assertions::assert_eq_size!(Dense, [u8; 128]);

// ----------------------------------------------------------------------------
// Insert

impl Tree {
    fn for_level(level: Level) -> Self {
        if level == LEAF_LEVEL {
            Self::Dense(Dense::default())
        } else {
            Self::Sparse(Sparse::default())
        }
    }

    fn increment(&mut self, level: Level, rel_addr: u64, inc: u64) {
        match self {
            Tree::Node(node) => {
                node.increment(level, rel_addr, inc);
            }
            Tree::Sparse(sparse) => {
                *self = std::mem::take(sparse).increment(level, rel_addr, inc);
            }
            Tree::Dense(dense) => {
                dense.increment(rel_addr, inc);
            }
        }
    }

    fn total_count(&self) -> u64 {
        match self {
            Tree::Node(node) => node.total_count(),
            Tree::Sparse(sparse) => sparse.total_count(),
            Tree::Dense(dense) => dense.total_count(),
        }
    }

    fn range_count(&self, level: Level, range: Range) -> u64 {
        match self {
            Tree::Node(node) => node.range_count(level, range),
            Tree::Sparse(sparse) => sparse.range_count(range),
            Tree::Dense(dense) => dense.range_count(range),
        }
    }

    fn collect_stats(&self, stats: &mut Stats) {
        match self {
            Tree::Node(node) => node.collect_stats(stats),
            Tree::Sparse(sparse) => sparse.collect_stats(stats),
            Tree::Dense(dense) => dense.collect_stats(stats),
        }
    }
}

impl Node {
    fn increment(&mut self, level: Level, rel_addr: u64, inc: u64) {
        debug_assert!(level != LEAF_LEVEL);
        let child_level = level - LEVEL_STEP;
        let (top_addr, bottom_addr) = split_address(level, rel_addr);
        self.children[top_addr as usize]
            .get_or_insert_with(|| Box::new(Tree::for_level(child_level)))
            .increment(child_level, bottom_addr, inc);
        self.total_count += inc;
    }

    fn total_count(&self) -> u64 {
        self.total_count
    }

    fn range_count(&self, level: Level, mut range: Range) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(level != LEAF_LEVEL);

        if true {
            let min_child = (range.min >> level) & 0b1;
            let max_child = ((range.max >> level) & 0b1).min(1);
            debug_assert!(
                min_child <= max_child,
                "Why where we called if we are not in range?"
            );

            if min_child == 0 && 1 <= max_child {
                return self.total_count;
            }

            let mut total_count = 0;

            if min_child == 0 {
                if let Some(child) = &self.children[0] {
                    total_count += child.range_count(level - LEVEL_STEP, range);
                }
            }
            if 1 <= max_child {
                if let Some(child) = &self.children[1] {
                    // slide range:
                    let child_size = 1 << level;
                    range = Range {
                        min: range.min.saturating_sub(child_size),
                        max: range.max.saturating_sub(child_size),
                    };
                    total_count += child.range_count(level - LEVEL_STEP, range);
                }
            }

            total_count
        } else {
            let min_child = (range.min >> level) & 0b1;
            let max_child = ((range.max >> level) & 0b1).min(1);
            debug_assert!(
                min_child <= max_child,
                "Why where we called if we are not in range?"
            );

            let range_includes_all_of_us = (min_child, max_child) == (0, 1);
            if range_includes_all_of_us {
                return self.total_count;
            }

            let child_size = 1 << level;

            let mut total_count = 0;

            for ci in 0..2 {
                if min_child <= ci {
                    if let Some(child) = &self.children[ci as usize] {
                        let child_level = level - LEVEL_STEP;
                        total_count += child.range_count(child_level, range);
                    }
                }

                // slide range:
                if range.max < child_size {
                    break; // the next child won't be in range
                }
                range.min = range.min.saturating_sub(child_size);
                range.max = range.max.saturating_sub(child_size);
            }

            total_count
        }
    }

    fn collect_stats(&self, stats: &mut Stats) {
        stats.num_nodes += 1;
        stats.node_bytes += std::mem::size_of::<Self>() as u64;
        for child in self.children.iter().flatten() {
            stats.node_child_count += 1;
            child.collect_stats(stats);
        }
    }
}

impl Sparse {
    #[must_use]
    fn overflow(self, level: Level) -> Node {
        debug_assert!(level != LEAF_LEVEL);

        // TODO: optimize for the case when everything goes into the left or right child

        let mut node = Node::default();
        for (key, count) in self.addr_counts {
            node.increment(level, key, count);
        }
        node
    }

    #[must_use]
    fn increment(mut self, level: Level, rel_addr: u64, inc: u64) -> Tree {
        let index = self
            .addr_counts
            .partition_point(|&(addr, _count)| addr < rel_addr);

        if let Some((addr, count)) = self.addr_counts.get_mut(index) {
            if *addr == rel_addr {
                *count += inc;
                return Tree::Sparse(self);
            }
        }

        const OVERFLOW_CUTOFF: usize = 32;
        if self.addr_counts.len() < OVERFLOW_CUTOFF {
            self.addr_counts.insert(index, (rel_addr, inc));
            Tree::Sparse(self)
        } else {
            let mut node = self.overflow(level);
            node.increment(level, rel_addr, inc);
            Tree::Node(node)
        }
    }

    fn total_count(&self) -> u64 {
        let mut total = 0;
        for (_key, count) in &self.addr_counts {
            total += *count;
        }
        total
    }

    fn range_count(&self, range: Range) -> u64 {
        // TODO: binary search
        let mut total = 0;
        for (key, count) in &self.addr_counts {
            if range.contains(*key) {
                total += *count;
            }
        }
        total
    }

    fn collect_stats(&self, stats: &mut Stats) {
        stats.num_sparse += 1;
        stats.sparse_bytes += std::mem::size_of::<Self>() as u64;
        stats.sparse_bytes_vec += self.addr_counts.len() as u64 * 16;
        stats.num_sparse_children += self.addr_counts.len() as u64;
    }
}

impl Dense {
    fn increment(&mut self, rel_addr: u64, inc: u64) {
        self.counts[rel_addr as usize] += inc as u32; // TODO: check overflow
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().sum::<u32>() as _ // TODO
    }

    fn range_count(&self, range: Range) -> u64 {
        debug_assert!(range.min <= range.max);
        let mut total_count: u64 = 0;
        for &count in &self.counts[range.min as usize..=(range.max as usize).min(3)] {
            total_count += count as u64;
        }
        total_count
    }

    #[allow(clippy::unused_self)]
    fn collect_stats(&self, stats: &mut Stats) {
        stats.num_dense += 1;
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_multiset() {
    let mut set = IntHistogram::default();
    for i in 0..=100 {
        debug_assert_eq!(set.total_count(), i);
        debug_assert_eq!(set.range_count(-10000..10000), i, "{:?}", set);
        let key = i as i64;
        set.increment(key, 1);
    }
}
