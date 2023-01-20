//! The histogram is implemented as a tree.
//!
//! The branches are based on the next few bits of the key (also known as the "address").

use smallvec::{smallvec, SmallVec};

use crate::{i64_key_from_u64_key, u64_key_from_i64_key, RangeI64, RangeU64};

// ----------------------------------------------------------------------------

/// How high up in the tree we are (where root is highest).
/// `1 << level` is the size of the range the next child.
/// So `1 << ROOT_LEVEL` is the size of the range of each children of the root.
type Level = u64;

// Uses 20x nodes with 8-way (3 bit) branching factor down to a final 16-way (4 bit) dense leaf.
// 20x 3-bit + 4-bit = 64 bit.
// level 1, 4, 7, …, 58, 61
// This uses about half the memory of 16-way branching, but is also half as fast.
// 5.7 B/dense entry
// 25-35 B/sparse entry

/// How many bits we progress in each [`BranchNode`]
const LEVEL_STEP: u64 = 3;
/// The level used for [`DenseLeaf`].
const BOTTOM_LEVEL: Level = 1;
/// Number of children in [`DenseLeaf`].
const NUM_CHILDREN_IN_DENSE: u64 = 16;

// High memory use, faster
// I believe we could trim this path to use much less memory
// by using dynamically sized nodes (no enum, no Vec/SmallVec),
// but that's left as an exercise for later.
// 9.6 B/dense entry
// 26-73 B/sparse entry

// /// How many bits we progress in each [`BranchNode`]
// const LEVEL_STEP: u64 = 4;
// /// The level used for [`DenseLeaf`].
// const BOTTOM_LEVEL: Level = 0;
// /// Number of children in [`DenseLeaf`].
// const NUM_CHILDREN_IN_DENSE: u64 = 16;

const ROOT_LEVEL: Level = 64 - LEVEL_STEP;
static_assertions::const_assert_eq!(ROOT_LEVEL + LEVEL_STEP, 64);
static_assertions::const_assert_eq!((ROOT_LEVEL - BOTTOM_LEVEL) % LEVEL_STEP, 0);
const NUM_NODE_STEPS: u64 = (ROOT_LEVEL - BOTTOM_LEVEL) / LEVEL_STEP;
#[allow(dead_code)] // used in static assert
const NUM_STEPS_IN_DENSE_LEAF: u64 = 64 - NUM_NODE_STEPS * LEVEL_STEP;
static_assertions::const_assert_eq!(1 << NUM_STEPS_IN_DENSE_LEAF, NUM_CHILDREN_IN_DENSE);

const ADDR_MASK: u64 = (1 << LEVEL_STEP) - 1;
const NUM_CHILDREN_IN_NODE: u64 = 1 << LEVEL_STEP;

/// When a [`SparseLeaf`] goes over this, it becomes a [`BranchNode`].
const MAX_SPARSE_LEAF_LEN: usize = 32;

fn child_level_and_size(level: Level) -> (Level, u64) {
    let child_level = level - LEVEL_STEP;
    let child_size = if child_level == 0 {
        NUM_CHILDREN_IN_DENSE
    } else {
        1 << level
    };
    (child_level, child_size)
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
    root: Node,
}

impl Default for Int64Histogram {
    fn default() -> Self {
        Self {
            root: Node::SparseLeaf(SparseLeaf::default()),
        }
    }
}

impl Int64Histogram {
    /// Increment the count for the given key.
    ///
    /// Incrementing with one is similar to inserting the key in a multi-set.
    pub fn increment(&mut self, key: i64, inc: u32) {
        self.root
            .increment(ROOT_LEVEL, u64_key_from_i64_key(key), inc);
    }

    /// Total count of all the buckets.
    ///
    /// NOTE: this is NOT the number of unique keys.
    pub fn total_count(&self) -> u64 {
        self.root.total_count()
    }

    /// What is the count of all the buckets in the given range?
    pub fn range_count(&self, range: impl std::ops::RangeBounds<i64>) -> u64 {
        let range = range_u64_from_range_bounds(range);
        if range.min <= range.max {
            self.root.range_count(0, ROOT_LEVEL, range)
        } else {
            0
        }
    }

    /// Iterate over a certain range, returning ranges that are at most `cutoff_size` long.
    ///
    /// To get all individual entries, use `cutoff_size<=1`.
    /// When `cutoff_size > 1` you will get approximate ranges, which may cover elements that has no count.
    ///
    /// For instance, inserting tow elements at `10` and `15` and setting a `cutoff_size=10`
    /// you may get a single range `[8, 16]` with the total count.
    pub fn range(&self, range: impl std::ops::RangeBounds<i64>, cutoff_size: u64) -> Iter<'_> {
        let range = range_u64_from_range_bounds(range);
        Iter {
            iter: TreeIterator {
                range,
                cutoff_size,
                stack: smallvec![NodeIterator {
                    level: ROOT_LEVEL,
                    abs_addr: 0,
                    node: &self.root,
                    index: 0,
                }],
            },
        }
    }

    /// Remove all data in the given range.
    ///
    /// Returns how much count was removed.
    ///
    /// Currently the implementation is optimized for the case of removing
    /// large continuous ranges.
    /// Removing many small, scattered ranges (e.g. individual elements)
    /// may cause performance problems!
    /// This can be remedied with some more code.
    pub fn remove(&mut self, range: impl std::ops::RangeBounds<i64>) -> u64 {
        let range = range_u64_from_range_bounds(range);
        self.root.remove(0, ROOT_LEVEL, range)
    }
}

/// An iterator over an [`Int64Histogram`].
///
/// Created with [`Int64Histogram::range`].
pub struct Iter<'a> {
    iter: TreeIterator<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = (RangeI64, u64);

    #[inline]
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

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug)]
enum Node {
    /// An inner node, addressed by the next few bits of the key/address.
    ///
    /// Never at the [`BOTTOM_LEVEL`] level.
    BranchNode(BranchNode),

    /// A list of `(key, count)` pairs.
    ///
    /// When this becomes too long, it will be converted into a [`BranchNode`].
    ///
    /// Never at the [`BOTTOM_LEVEL`] level.
    SparseLeaf(SparseLeaf),

    /// Optimization for dense histograms (entries at `N, N+1, N+2, …`).
    ///
    /// Always at the [`BOTTOM_LEVEL`] level.
    DenseLeaf(DenseLeaf),
}
static_assertions::assert_eq_size!(Node, (u64, BranchNode), [u8; 80]); // 8-way tree

#[derive(Clone, Debug, Default)]
struct BranchNode {
    /// Very important optimization
    total_count: u64,

    /// The index is the next few bits of the key
    children: [Option<Box<Node>>; NUM_CHILDREN_IN_NODE as usize],
}

#[derive(Clone, Debug, Default)]
struct SparseLeaf {
    /// Two vectors of equal lengths,
    /// making up (addr, count) pairs,
    /// sorted by `addr`.
    addrs: SmallVec<[u64; 3]>,
    counts: SmallVec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, Default)]
struct DenseLeaf {
    /// The last bits of the address, mapped to their counts
    counts: [u32; NUM_CHILDREN_IN_DENSE as usize],
}

// ----------------------------------------------------------------------------
// Insert

impl Node {
    /// The default node for a certain level.
    fn for_level(level: Level) -> Self {
        if level == BOTTOM_LEVEL {
            Self::DenseLeaf(DenseLeaf::default())
        } else {
            Self::SparseLeaf(SparseLeaf::default())
        }
    }

    fn increment(&mut self, level: Level, addr: u64, inc: u32) {
        match self {
            Node::BranchNode(node) => {
                node.increment(level, addr, inc);
            }
            Node::SparseLeaf(sparse) => {
                *self = std::mem::take(sparse).increment(level, addr, inc);
            }
            Node::DenseLeaf(dense) => {
                dense.increment(addr, inc);
            }
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Node::BranchNode(node) => node.is_empty(),
            Node::SparseLeaf(sparse) => sparse.is_empty(),
            Node::DenseLeaf(dense) => dense.is_empty(),
        }
    }

    fn total_count(&self) -> u64 {
        match self {
            Node::BranchNode(node) => node.total_count(),
            Node::SparseLeaf(sparse) => sparse.total_count(),
            Node::DenseLeaf(dense) => dense.total_count(),
        }
    }

    fn range_count(&self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        match self {
            Node::BranchNode(node) => node.range_count(my_addr, my_level, range),
            Node::SparseLeaf(sparse) => sparse.range_count(range),
            Node::DenseLeaf(dense) => dense.range_count(my_addr, range),
        }
    }

    /// Returns how much the total count decreased by.
    fn remove(&mut self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        match self {
            Node::BranchNode(node) => {
                let count_loss = node.remove(my_addr, my_level, range);
                if node.is_empty() {
                    *self = Node::SparseLeaf(SparseLeaf::default());
                }
                // TODO(emilk): if we only have leaf children (sparse or dense)
                // and the number of keys in all of them is less then `MAX_SPARSE_LEAF_LEN`,
                // then we should convert this BranchNode into a SparseLeaf.
                count_loss
            }
            Node::SparseLeaf(sparse) => sparse.remove(range),
            Node::DenseLeaf(dense) => dense.remove(my_addr, range),
        }
    }
}

impl BranchNode {
    fn increment(&mut self, level: Level, addr: u64, inc: u32) {
        debug_assert!(level != BOTTOM_LEVEL);
        let child_level = level - LEVEL_STEP;
        let top_addr = (addr >> level) & ADDR_MASK;
        self.children[top_addr as usize]
            .get_or_insert_with(|| Box::new(Node::for_level(child_level)))
            .increment(child_level, addr, inc);
        self.total_count += inc as u64;
    }

    fn is_empty(&self) -> bool {
        self.total_count == 0
    }

    fn total_count(&self) -> u64 {
        self.total_count
    }

    fn range_count(&self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(my_level != BOTTOM_LEVEL);

        let (child_level, child_size) = child_level_and_size(my_level);

        let mut total_count = 0;

        for ci in 0..NUM_CHILDREN_IN_NODE {
            let child_addr = my_addr + ci * child_size;
            let child_range = RangeU64::new(child_addr, child_addr + (child_size - 1));
            if range.intersects(child_range) {
                if let Some(child) = &self.children[ci as usize] {
                    if range.contains_all_of(child_range) {
                        total_count += child.total_count();
                    } else {
                        total_count += child.range_count(child_addr, child_level, range);
                    }
                }
            }
        }

        total_count
    }

    /// Returns how much the total count decreased by.
    #[must_use]
    fn remove(&mut self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(my_level != BOTTOM_LEVEL);

        let mut count_loss = 0;
        let (child_level, child_size) = child_level_and_size(my_level);

        for ci in 0..NUM_CHILDREN_IN_NODE {
            let child_addr = my_addr + ci * child_size;
            let child_range = RangeU64::new(child_addr, child_addr + (child_size - 1));
            if range.intersects(child_range) {
                if let Some(child) = &mut self.children[ci as usize] {
                    if range.contains_all_of(child_range) {
                        count_loss += child.total_count();
                        self.children[ci as usize] = None;
                    } else {
                        count_loss += child.remove(child_addr, child_level, range);
                        if child.is_empty() {
                            self.children[ci as usize] = None;
                        }
                    }
                }
            }
        }

        self.total_count -= count_loss;

        count_loss
    }
}

impl SparseLeaf {
    #[must_use]
    fn overflow(self, level: Level) -> BranchNode {
        debug_assert!(level != BOTTOM_LEVEL);

        let mut node = BranchNode::default();
        for (key, count) in self.addrs.iter().zip(&self.counts) {
            node.increment(level, *key, *count);
        }
        node
    }

    #[must_use]
    fn increment(mut self, level: Level, abs_addr: u64, inc: u32) -> Node {
        let index = self.addrs.partition_point(|&addr| addr < abs_addr);

        if let (Some(addr), Some(count)) = (self.addrs.get_mut(index), self.counts.get_mut(index)) {
            if *addr == abs_addr {
                *count += inc;
                return Node::SparseLeaf(self);
            }
        }

        if self.addrs.len() < MAX_SPARSE_LEAF_LEN {
            self.addrs.insert(index, abs_addr);
            self.counts.insert(index, inc);
            Node::SparseLeaf(self)
        } else {
            let mut node = self.overflow(level);
            node.increment(level, abs_addr, inc);
            Node::BranchNode(node)
        }
    }

    fn is_empty(&self) -> bool {
        self.addrs.is_empty()
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

    /// Returns how much the total count decreased by.
    #[must_use]
    fn remove(&mut self, range: RangeU64) -> u64 {
        debug_assert_eq!(self.addrs.len(), self.counts.len());

        let mut count_loss = 0;
        for (key, count) in self.addrs.iter().zip(&mut self.counts) {
            if range.contains(*key) {
                count_loss += *count as u64;
                *count = 0;
            }
        }

        self.addrs.retain(|addr| !range.contains(*addr));
        self.counts.retain(|count| *count > 0);
        debug_assert_eq!(self.addrs.len(), self.counts.len());
        count_loss
    }
}

impl DenseLeaf {
    fn increment(&mut self, abs_addr: u64, inc: u32) {
        self.counts[(abs_addr & (NUM_CHILDREN_IN_DENSE - 1)) as usize] += inc;
    }

    fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    fn range_count(&self, my_addr: u64, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        let mut total_count = 0;
        for (i, count) in self.counts.iter().enumerate() {
            if range.contains(my_addr + i as u64) {
                total_count += *count as u64;
            }
        }
        total_count
    }

    /// Returns how much the total count decreased by.
    #[must_use]
    fn remove(&mut self, my_addr: u64, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        let mut count_loss = 0;
        for (i, count) in self.counts.iter_mut().enumerate() {
            if range.contains(my_addr + i as u64) {
                count_loss += *count as u64;
                *count = 0;
            }
        }
        count_loss
    }
}

// ----------------------------------------------------------------------------

struct TreeIterator<'a> {
    /// Only returns things in this range
    range: RangeU64,

    /// You can stop recursing when you've reached this size
    cutoff_size: u64,

    stack: SmallVec<[NodeIterator<'a>; (NUM_NODE_STEPS + 1) as usize]>,
}

struct NodeIterator<'a> {
    level: Level,
    abs_addr: u64,
    node: &'a Node,
    index: usize,
}

impl<'a> Iterator for TreeIterator<'a> {
    /// Am inclusive range, and the total count in that range.
    type Item = (RangeU64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        'outer: while let Some(it) = self.stack.last_mut() {
            match it.node {
                Node::BranchNode(node) => {
                    let (child_level, child_size) = child_level_and_size(it.level);

                    while it.index < NUM_CHILDREN_IN_NODE as _ {
                        let child_addr = it.abs_addr + child_size * it.index as u64;
                        let child_range = RangeU64 {
                            min: child_addr,
                            max: child_addr + (child_size - 1),
                        };
                        if self.range.intersects(child_range) {
                            if let Some(Some(child)) = node.children.get(it.index) {
                                it.index += 1;

                                if child_size <= self.cutoff_size
                                    && self.range.contains_all_of(child_range)
                                {
                                    let count = child.total_count();
                                    return Some((child_range, count));
                                }

                                self.stack.push(NodeIterator {
                                    level: child_level,
                                    abs_addr: child_addr,
                                    node: child,
                                    index: 0,
                                });
                                continue 'outer;
                            }
                        }
                        it.index += 1;
                    }
                }
                Node::SparseLeaf(sparse) => {
                    while let (Some(abs_addr), Some(count)) =
                        (sparse.addrs.get(it.index), sparse.counts.get(it.index))
                    {
                        it.index += 1;
                        if self.range.contains(*abs_addr) {
                            return Some((RangeU64::single(*abs_addr), *count as u64));
                        }
                    }
                }
                Node::DenseLeaf(dense) => {
                    while let Some(count) = dense.counts.get(it.index) {
                        let abs_addr = it.abs_addr + it.index as u64;
                        it.index += 1;
                        if 0 < *count && self.range.contains(abs_addr) {
                            return Some((RangeU64::single(abs_addr), *count as u64));
                        }
                    }
                }
            }
            self.stack.pop();
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

    assert_eq!(set.range(.., 1).collect::<Vec<_>>(), expected_ranges);
    assert_eq!(set.range(..10, 1).count(), 10);
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

    assert_eq!(set.range(.., 1).collect::<Vec<_>>(), expected_ranges);
    assert_eq!(set.range(..10 * spacing, 1).count(), 10);
}

#[test]
fn test_two_dense_ranges() {
    let mut set = Int64Histogram::default();
    for i in 0..100 {
        set.increment(i, 1);
        set.increment(10_000 + i, 1);
        set.increment(20_000 + i, 1);
    }

    assert_eq!(set.range(..15_000, 1000).count(), 2);

    assert_eq!(set.total_count(), 300);
    assert_eq!(set.remove(..10_020), 120);
    assert_eq!(set.total_count(), 180);
}

#[test]
fn test_two_sparse_ranges() {
    let mut set = Int64Histogram::default();
    let mut should_contain = vec![];
    let mut should_not_contain = vec![];
    for i in 0..100 {
        let a = -1_000_000_000 + i * 1_000;
        let b = (i - 50) * 1_000;
        let c = 1_000_000_000 + i * 1_000;
        set.increment(a, 1);
        set.increment(b, 1);
        set.increment(c, 1);

        should_contain.push(a);
        should_contain.push(b);
        should_not_contain.push(c);
    }

    let ranges = set.range(..1_000_000_000, 1_000_000).collect::<Vec<_>>();

    assert!(ranges.len() < 10, "We shouldn't get too many ranges");

    let ranges_contains = |value| ranges.iter().any(|(range, _count)| range.contains(value));

    for value in should_contain {
        assert!(ranges_contains(value));
    }
    for value in should_not_contain {
        assert!(!ranges_contains(value));
    }

    assert_eq!(set.total_count(), 300);
    assert_eq!(set.remove(..0), 150);
    assert_eq!(set.total_count(), 150);
}

#[test]
fn test_removal() {
    let mut set = Int64Histogram::default();
    set.increment(i64::MAX, 1);
    set.increment(i64::MAX - 1, 2);
    set.increment(i64::MAX - 2, 3);
    set.increment(i64::MIN + 2, 3);
    set.increment(i64::MIN + 1, 2);
    set.increment(i64::MIN, 1);

    debug_assert_eq!(set.range_count((i64::MAX - 1)..=i64::MAX), 3);
    debug_assert_eq!(
        set.range(0.., 1).collect::<Vec<_>>(),
        vec![
            (RangeI64::single(i64::MAX - 2), 3),
            (RangeI64::single(i64::MAX - 1), 2),
            (RangeI64::single(i64::MAX), 1),
        ]
    );

    set.remove(i64::MAX..=i64::MAX);

    debug_assert_eq!(
        set.range(.., 1).collect::<Vec<_>>(),
        vec![
            (RangeI64::single(i64::MIN), 1),
            (RangeI64::single(i64::MIN + 1), 2),
            (RangeI64::single(i64::MIN + 2), 3),
            (RangeI64::single(i64::MAX - 2), 3),
            (RangeI64::single(i64::MAX - 1), 2),
        ]
    );

    set.remove(i64::MIN..=(i64::MAX - 2));

    debug_assert_eq!(
        set.range(.., 1).collect::<Vec<_>>(),
        vec![(RangeI64::single(i64::MAX - 1), 2),]
    );
}
