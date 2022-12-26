//! Uses 16 level of nodes with 16-way (4 bit) branching factor.
//!
//! This uses a lot of of memory (2x of [`crate::tree8`].
//! I think there is an opportunity to optimize the memory use of [`crate::tree16`]
//! with sparse nodes, and dynamically sized [`Tree`].
//! The first byte could be the enum discriminant, followed by the capacity and size of
//! either the sparse node or sparse leaf, followed by enough memory to contain it.

use crate::u64_key_from_i64_key;

// ----------------------------------------------------------------------------

type Level = u64;
const ROOT_LEVEL: Level = 60;
const LEAF_LEVEL: Level = 0;
const LEVEL_STEP: u64 = 4;
// const CHILDREN_PER_LEVEL: u64 = 16;

// level 0, 4, 8, 16, …, 56, 60
// (1 << level) is the size of the range the next child

#[inline(always)]
fn split_address(level: Level, addr: u64) -> (u64, u64) {
    let top = (addr >> level) & 0b1111;
    let bottom = addr & ((1 << level) - 1);
    (top, bottom)
}

// ----------------------------------------------------------------------------
// High-level API

/// A histogram, mapping [`i64`] key to a [`u64`] count
/// optimizing for very fast range-queries.
#[derive(Clone, Debug)]
pub struct IntHistogram {
    tree: Tree,
}

impl Default for IntHistogram {
    fn default() -> Self {
        Self {
            tree: Tree::Sparse(Sparse::default()),
        }
    }
}

impl IntHistogram {
    /// Insert in multi-set.
    ///
    /// Increments the count of the given bucket.
    pub fn increment(&mut self, key: i64, inc: u32) {
        self.tree
            .increment(ROOT_LEVEL, u64_key_from_i64_key(key), inc);
    }

    /// Total count in all the buckets.
    ///
    /// NOTE: this is not the number of unique keys, but cardinality of the multiset.
    pub fn total_count(&self) -> u64 {
        self.tree.total_count()
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
        self.tree.range_count(
            ROOT_LEVEL,
            Range {
                min: u64_key_from_i64_key(min),
                max: u64_key_from_i64_key(max),
            },
        )
    }

    // fn remove_all_in_range(&mut self, min: i64, max: i64)
}

// ----------------------------------------------------------------------------
// Low-level data structure.
// All internal addressed are relative.

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
static_assertions::assert_eq_size!(Tree, (u64, Node), [u8; 144]);

#[derive(Clone, Debug, Default)]
struct Node {
    /// Very important optimization
    total_count: u64,

    /// The index is the next 4 bits of the key
    children: [Option<Box<Tree>>; 16],
}
static_assertions::assert_eq_size!(Node, [u8; 136]);

#[derive(Clone, Debug, Default)]
struct Sparse {
    /// Sorted (addr, count) pairs
    addrs: smallvec::SmallVec<[u64; 9]>,
    counts: smallvec::SmallVec<[u32; 9]>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Dense {
    /// The last 4 bits of the address, mapped to their counts
    counts: [u32; 16],
}

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

    fn increment(&mut self, level: Level, rel_addr: u64, inc: u32) {
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
}

impl Node {
    fn increment(&mut self, level: Level, rel_addr: u64, inc: u32) {
        debug_assert!(level != LEAF_LEVEL);
        let child_level = level - LEVEL_STEP;
        let (top_addr, bottom_addr) = split_address(level, rel_addr);
        self.children[top_addr as usize]
            .get_or_insert_with(|| Box::new(Tree::for_level(child_level)))
            .increment(child_level, bottom_addr, inc);
        self.total_count += inc as u64;
    }

    fn total_count(&self) -> u64 {
        self.total_count
    }

    fn range_count(&self, level: Level, mut range: Range) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(level != LEAF_LEVEL);

        let min_child = (range.min >> level) & 0b1111;
        let max_child = ((range.max >> level) & 0b1111).min(15);
        debug_assert!(
            min_child <= max_child,
            "Why where we called if we are not in range?"
        );

        let range_includes_all_of_us = (min_child, max_child) == (0, 15);
        if range_includes_all_of_us {
            return self.total_count;
        }

        let child_size = 1 << level;

        let mut total_count = 0;

        for ci in 0..16 {
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

impl Sparse {
    #[must_use]
    fn overflow(self, level: Level) -> Node {
        debug_assert!(level != LEAF_LEVEL);

        let mut node = Node::default();
        for (key, count) in self.addrs.iter().zip(&self.counts) {
            node.increment(level, *key, *count);
        }
        node
    }

    #[must_use]
    fn increment(mut self, level: Level, rel_addr: u64, inc: u32) -> Tree {
        let index = self.addrs.partition_point(|&addr| addr < rel_addr);

        if let (Some(addr), Some(count)) = (self.addrs.get_mut(index), self.counts.get_mut(index)) {
            if *addr == rel_addr {
                *count += inc;
                return Tree::Sparse(self);
            }
        }

        const OVERFLOW_CUTOFF: usize = 32;
        if self.addrs.len() < OVERFLOW_CUTOFF {
            self.addrs.insert(index, rel_addr);
            self.counts.insert(index, inc);
            Tree::Sparse(self)
        } else {
            let mut node = self.overflow(level);
            node.increment(level, rel_addr, inc);
            Tree::Node(node)
        }
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    fn range_count(&self, range: Range) -> u64 {
        let mut total = 0;
        for (key, count) in self.addrs.iter().zip(&self.counts) {
            if range.contains(*key) {
                total += *count as u64;
            }
        }
        total
    }
}

impl Dense {
    fn increment(&mut self, rel_addr: u64, inc: u32) {
        self.counts[rel_addr as usize] += inc;
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    fn range_count(&self, range: Range) -> u64 {
        debug_assert!(range.min <= range.max);
        let mut total_count = 0;
        for &count in &self.counts[range.min as usize..=(range.max as usize).min(15)] {
            total_count += count as u64;
        }
        total_count
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_multiset() {
    let mut set = IntHistogram::default();
    for i in 0..=100 {
        debug_assert_eq!(set.total_count(), i);
        debug_assert_eq!(set.range_count(-10000..10000), i);
        let key = i as i64;
        set.increment(key, 1);
    }
}
