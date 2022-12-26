//! Uses 20x nodes with 8-way (3 bit) branching factor down to a final 16-way (4 bit) dense leaf.
//! 20x 3-bit + 4-bit = 64 bit.
//!
//! This uses about half the memory of [`crate::tree16`], but is also half as fast.
//! I believe we could trim [`crate::tree16`] to use much less memory,
//! by using dynamically sized nodes, but that's left as an exercise for later.

use crate::{i64_key_from_u64_key, u64_key_from_i64_key, RangeI64, RangeU64};

// ----------------------------------------------------------------------------

type Level = u64;
const ROOT_LEVEL: Level = 61;
const LEAF_LEVEL: Level = 1;
const LEVEL_STEP: u64 = 3;
const ADDR_MASK: u64 = 0b111;
const NUM_CHILDREN_IN_NODE: u64 = 8;
const NUM_CHILDREN_IN_DENSE: u64 = 16;

// level 1, 4, 7, …, 58, 61
// (1 << level) is the size of the range the next child

#[inline(always)]
fn split_address(level: Level, addr: u64) -> (u64, u64) {
    let top = (addr >> level) & ADDR_MASK;
    let bottom = addr & ((1 << level) - 1);
    (top, bottom)
}

fn range_u64_from_range_bounds(range: impl std::ops::RangeBounds<i64>) -> RangeU64 {
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
    RangeU64 {
        min: u64_key_from_i64_key(min),
        max: u64_key_from_i64_key(max),
    }
}

// ----------------------------------------------------------------------------
// High-level API

/// A histogram, mapping [`i64`] key to a [`u64`] count
/// optimizing for very fast range-queries.
#[derive(Clone, Debug)]
pub struct Int64Histogram {
    tree: Tree,
}

impl Default for Int64Histogram {
    fn default() -> Self {
        Self {
            tree: Tree::Sparse(Sparse::default()),
        }
    }
}

impl Int64Histogram {
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
        let range = range_u64_from_range_bounds(range);
        if range.min <= range.max {
            self.tree.range_count(ROOT_LEVEL, range)
        } else {
            0
        }
    }

    pub fn iter(&self, range: impl std::ops::RangeBounds<i64>) -> Iter<'_> {
        let range = range_u64_from_range_bounds(range);
        Iter {
            iter: TreeIterator {
                range,
                stack: vec![NodeIterator {
                    level: ROOT_LEVEL,
                    abs_addr: 0,
                    tree: &self.tree,
                    index: 0,
                }],
            },
        }
    }
}

pub struct Iter<'a> {
    iter: TreeIterator<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (RangeI64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(range, count)| {
            (
                RangeI64 {
                    min: i64_key_from_u64_key(range.min),
                    max: i64_key_from_u64_key(range.max),
                },
                count,
            )
        })
    }
}

// ----------------------------------------------------------------------------
// Low-level data structure.
// All internal addressed are relative.

/// 136 bytes large
/// Has sixteen levels. The root is level 15, the leaves level 0.
#[derive(Clone, Debug)]
enum Tree {
    Node(Node),
    Sparse(Sparse),
    Dense(Dense),
}
static_assertions::assert_eq_size!(Tree, (u64, Node), [u8; 80]);

#[derive(Clone, Debug, Default)]
struct Node {
    /// Very important optimization
    total_count: u64,

    /// The index is the next 4 bits of the key
    children: [Option<Box<Tree>>; NUM_CHILDREN_IN_NODE as usize],
}
static_assertions::assert_eq_size!(Node, [u8; 72]);

#[derive(Clone, Debug, Default)]
struct Sparse {
    /// Sorted (addr, count) pairs
    addrs: smallvec::SmallVec<[u64; 3]>,
    counts: smallvec::SmallVec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Dense {
    /// The last bits of the address, mapped to their counts
    counts: [u32; NUM_CHILDREN_IN_DENSE as usize],
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

    fn range_count(&self, level: Level, range: RangeU64) -> u64 {
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

    fn range_count(&self, level: Level, mut range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(level != LEAF_LEVEL);

        let min_child = (range.min >> level) & ADDR_MASK;
        let max_child = ((range.max >> level) & ADDR_MASK).min(NUM_CHILDREN_IN_NODE - 1);
        debug_assert!(
            min_child <= max_child,
            "Why where we called if we are not in range?"
        );

        let range_includes_all_of_us = (min_child, max_child) == (0, NUM_CHILDREN_IN_NODE - 1);
        if range_includes_all_of_us {
            return self.total_count;
        }

        let child_level = level - LEVEL_STEP;

        let child_size = if child_level == 0 {
            NUM_CHILDREN_IN_DENSE
        } else {
            1 << level
        };

        let mut total_count = 0;

        for ci in 0..NUM_CHILDREN_IN_NODE {
            if min_child <= ci {
                if let Some(child) = &self.children[ci as usize] {
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

    fn range_count(&self, range: RangeU64) -> u64 {
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

    fn range_count(&self, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        let mut total_count = 0;
        for &count in
            &self.counts[range.min as usize..=(range.max.min(NUM_CHILDREN_IN_DENSE - 1) as usize)]
        {
            total_count += count as u64;
        }
        total_count
    }
}

// ----------------------------------------------------------------------------

struct TreeIterator<'a> {
    /// Only returns things in this range
    range: RangeU64,
    stack: Vec<NodeIterator<'a>>,
}

struct NodeIterator<'a> {
    level: Level,
    abs_addr: u64,
    tree: &'a Tree,
    index: usize,
}

impl<'a> Iterator for TreeIterator<'a> {
    /// Am inclusive range, and the total count in that range.
    type Item = (RangeU64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        'outer: while let Some(it) = self.stack.last_mut() {
            match it.tree {
                Tree::Node(node) => {
                    let child_level = it.level - LEVEL_STEP;

                    let child_size = if child_level == 0 {
                        NUM_CHILDREN_IN_DENSE
                    } else {
                        1 << it.level
                    };

                    while it.index < NUM_CHILDREN_IN_NODE as _ {
                        let abs_addr = it.abs_addr + child_size * it.index as u64;
                        let child_range = RangeU64 {
                            min: abs_addr,
                            max: abs_addr + (child_size - 1),
                        };
                        if self.range.intersects(child_range) {
                            if let Some(Some(child)) = node.children.get(it.index) {
                                it.index += 1;
                                self.stack.push(NodeIterator {
                                    level: child_level,
                                    abs_addr,
                                    tree: child,
                                    index: 0,
                                });
                                continue 'outer;
                            }
                        }
                        it.index += 1;
                    }
                    self.stack.pop();
                }
                Tree::Sparse(sparse) => {
                    while let (Some(rel_addr), Some(count)) =
                        (sparse.addrs.get(it.index), sparse.counts.get(it.index))
                    {
                        it.index += 1;
                        let abs_addr = it.abs_addr + *rel_addr;
                        if self.range.contains(abs_addr) {
                            return Some((RangeU64::single(abs_addr), *count as u64));
                        }
                    }
                    self.stack.pop();
                }
                Tree::Dense(dense) => {
                    while let Some(count) = dense.counts.get(it.index) {
                        let abs_addr = it.abs_addr + it.index as u64;
                        it.index += 1;
                        if 0 < *count && self.range.contains(abs_addr) {
                            return Some((RangeU64::single(abs_addr), *count as u64));
                        }
                    }
                    self.stack.pop();
                }
            }
        }
        None
    }
}

// ----------------------------------------------------------------------------

#[test]
fn test_dense() {
    let mut set = Int64Histogram::default();
    let mut expected_ranges = vec![];
    for i in 0..100 {
        debug_assert_eq!(set.total_count(), i);
        debug_assert_eq!(set.range_count(-10000..10000), i);
        let key = i as i64;
        set.increment(key, 1);

        expected_ranges.push((RangeI64::single(key), 1));
    }

    assert_eq!(set.iter(..).collect::<Vec<_>>(), expected_ranges);
    assert_eq!(set.iter(..10).count(), 10);
}

#[test]
fn test_sparse() {
    let inc = 2;
    let spacing = 1_000_000;
    let mut set = Int64Histogram::default();
    let mut expected_ranges = vec![];
    for i in 0..100 {
        debug_assert_eq!(set.total_count(), inc * i);
        debug_assert_eq!(set.range_count(-10000..10000 * spacing), inc * i);
        let key = i as i64 * spacing;
        set.increment(key, inc as u32);
        expected_ranges.push((RangeI64::single(key), inc));
    }

    assert_eq!(set.iter(..).collect::<Vec<_>>(), expected_ranges);
    assert_eq!(set.iter(..10 * spacing).count(), 10);
}
