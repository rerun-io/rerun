//! The histogram is implemented as a trie.
//!
//! Each node in the trie stores a count of a key/address sharing a prefix up to `depth * LEVEL_STEP` bits.
//! The key/address is always 64 bits.
//!
//! There are branch nodes, and two types of leaf nodes: dense, and sparse.
//! Dense leaves are only found at the very bottom of the trie.

use smallvec::{SmallVec, smallvec};

use crate::{RangeI64, RangeU64, i64_key_from_u64_key, u64_key_from_i64_key};

// ----------------------------------------------------------------------------

/// How high up in the tree we are (where root is highest).
/// `1 << level` is the size of the range the next child.
/// So `1 << ROOT_LEVEL` is the size of the range of each children of the root.
type Level = u64;

// ----------------------------------------------------------------------------

#[expect(dead_code)]
mod small_and_slow {
    #[allow(clippy::allow_attributes, clippy::wildcard_imports)] // for the sake of doclinks
    use super::*;

    // Uses 20x nodes with 8-way (3 bit) branching factor down to a final 16-way (4 bit) dense leaf.
    // 20x 3-bit + 4-bit = 64 bit.
    // level 1, 4, 7, …, 58, 61
    // This uses about half the memory of 16-way branching, but is also half as fast.
    // 5.7 B/dense entry
    // 25-35 B/sparse entry

    /// How many bits we progress in each [`BranchNode`]
    pub const LEVEL_STEP: u64 = 3;

    /// The level used for [`DenseLeaf`].
    pub const BOTTOM_LEVEL: Level = 1;

    /// Number of children in [`DenseLeaf`].
    pub const NUM_CHILDREN_IN_DENSE: u64 = 16;
}

// ----------------------------------------------------------------------------

mod large_and_fast {
    #[allow(clippy::allow_attributes, clippy::wildcard_imports)] // for the sake of doclinks
    use super::*;

    // High memory use, faster
    // I believe we could trim this path to use much less memory
    // by using dynamically sized nodes (no enum, no Vec/SmallVec),
    // but that's left as an exercise for later.
    // 9.6 B/dense entry
    // 26-73 B/sparse entry

    /// How many bits we progress in each [`BranchNode`]
    pub const LEVEL_STEP: u64 = 4;

    /// The level used for [`DenseLeaf`].
    pub const BOTTOM_LEVEL: Level = 0;

    /// Number of children in [`DenseLeaf`].
    pub const NUM_CHILDREN_IN_DENSE: u64 = 16;
}

use large_and_fast::{BOTTOM_LEVEL, LEVEL_STEP, NUM_CHILDREN_IN_DENSE};

// ----------------------------------------------------------------------------

const ROOT_LEVEL: Level = 64 - LEVEL_STEP;
static_assertions::const_assert_eq!(ROOT_LEVEL + LEVEL_STEP, 64);
static_assertions::const_assert_eq!((ROOT_LEVEL - BOTTOM_LEVEL) % LEVEL_STEP, 0);
const NUM_NODE_STEPS: u64 = (ROOT_LEVEL - BOTTOM_LEVEL) / LEVEL_STEP;
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
        if inc != 0 {
            self.root
                .increment(ROOT_LEVEL, u64_key_from_i64_key(key), inc);
        }
    }

    /// Decrement the count for the given key.
    ///
    /// The decrement is saturating.
    ///
    /// Returns how much was actually decremented (found).
    /// If the returned value is less than the given value,
    /// it means that the key was either no found, or had a lower count.
    pub fn decrement(&mut self, key: i64, dec: u32) -> u32 {
        if dec == 0 {
            0
        } else {
            self.root
                .decrement(ROOT_LEVEL, u64_key_from_i64_key(key), dec)
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

    /// Is the total count zero?
    ///
    /// Note that incrementing a key with zero is a no-op and
    /// will leave an empty histogram still empty.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    /// Total count of all the buckets.
    ///
    /// NOTE: this is NOT the number of unique keys.
    pub fn total_count(&self) -> u64 {
        self.root.total_count()
    }

    /// Lowest key with a non-zero count.
    pub fn min_key(&self) -> Option<i64> {
        self.root.min_key(0, ROOT_LEVEL).map(i64_key_from_u64_key)
    }

    /// Highest key with a non-zero count.
    pub fn max_key(&self) -> Option<i64> {
        self.root.max_key(0, ROOT_LEVEL).map(i64_key_from_u64_key)
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
    ///
    /// When `cutoff_size > 1` you MAY get ranges which include keys that has no count.
    /// However, the ends (min/max) of all returned ranges will be keys with a non-zero count.
    ///
    /// In other words, gaps in the key-space smaller than `cutoff_size` MAY be ignored by this iterator.
    ///
    /// For example, inserting two elements at `10` and `15` and setting a `cutoff_size=10`
    /// you may get a single range `[10, 15]` with the total count.
    /// You may also get two ranges of `[10, 10]` and `[15, 15]`.
    ///
    /// A larger `cutoff_size` will generally yield fewer ranges, and will be faster.
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

    /// Find the next key greater than the given time.
    ///
    /// If found, returns that key. Otherwise wraps around and returns the minimum key.
    /// Returns `None` only if the histogram is empty.
    pub fn next_key_after(&self, time: i64) -> Option<i64> {
        // Use cutoff_size=1 to get individual keys
        if let Some((range, _)) = self
            .range(
                (std::ops::Bound::Excluded(time), std::ops::Bound::Unbounded),
                1,
            )
            .next()
        {
            Some(range.min)
        } else {
            // Wrap around to the minimum key
            self.min_key()
        }
    }

    /// Find the previous key less than the given time.
    ///
    /// If found, returns that key. Otherwise wraps around and returns the maximum key.
    /// Returns `None` only if the histogram is empty.
    pub fn prev_key_before(&self, time: i64) -> Option<i64> {
        // Fast path: if the maximum key is less than time, we can return it directly
        // This is O(log n) and avoids iterating through ranges
        if let Some(max) = self.max_key() {
            if max < time {
                return Some(max);
            }
        } else {
            // Empty histogram
            return None;
        }

        // Optimization: Use a larger cutoff_size to reduce the number of ranges we iterate through.
        // With cutoff_size=1024, we get ranges up to 1024 keys long, which dramatically reduces
        // the number of iterations for sparse histograms.
        // According to the documentation, the ends (min/max) of returned ranges are guaranteed
        // to be keys with non-zero count, so the max of the last range is the correct answer.
        let mut last_range_max = None;
        for (range, _) in self.range(
            (std::ops::Bound::Unbounded, std::ops::Bound::Excluded(time)),
            1024,
        ) {
            last_range_max = Some(range.max);
        }

        last_range_max.or_else(|| {
            // No keys before time, wrap around to max
            self.max_key()
        })
    }
}

/// An iterator over an [`Int64Histogram`].
///
/// Created with [`Int64Histogram::range`].
pub struct Iter<'a> {
    iter: TreeIterator<'a>,
}

impl Iterator for Iter<'_> {
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

    /// The count may never be zero.
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
            Self::BranchNode(node) => {
                node.increment(level, addr, inc);
            }
            Self::SparseLeaf(sparse) => {
                *self = std::mem::take(sparse).increment(level, addr, inc);
            }
            Self::DenseLeaf(dense) => {
                dense.increment(addr, inc);
            }
        }
    }

    /// Returns how much the total count decreased by.
    #[must_use]
    fn decrement(&mut self, level: Level, addr: u64, dec: u32) -> u32 {
        match self {
            Self::BranchNode(node) => {
                let count_loss = node.decrement(level, addr, dec);
                if node.is_empty() {
                    *self = Self::SparseLeaf(SparseLeaf::default());
                }
                // TODO(emilk): if we only have leaf children (sparse or dense)
                // and the number of keys in all of them is less then `MAX_SPARSE_LEAF_LEN`,
                // then we should convert this BranchNode into a SparseLeaf.
                count_loss
            }
            Self::SparseLeaf(sparse) => sparse.decrement(addr, dec),
            Self::DenseLeaf(dense) => dense.decrement(addr, dec),
        }
    }

    /// Returns how much the total count decreased by.
    fn remove(&mut self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        match self {
            Self::BranchNode(node) => {
                let count_loss = node.remove(my_addr, my_level, range);
                if node.is_empty() {
                    *self = Self::SparseLeaf(SparseLeaf::default());
                }
                // TODO(emilk): if we only have leaf children (sparse or dense)
                // and the number of keys in all of them is less then `MAX_SPARSE_LEAF_LEN`,
                // then we should convert this BranchNode into a SparseLeaf.
                count_loss
            }
            Self::SparseLeaf(sparse) => sparse.remove(range),
            Self::DenseLeaf(dense) => dense.remove(my_addr, range),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::BranchNode(node) => node.is_empty(),
            Self::SparseLeaf(sparse) => sparse.is_empty(),
            Self::DenseLeaf(dense) => dense.is_empty(),
        }
    }

    fn total_count(&self) -> u64 {
        match self {
            Self::BranchNode(node) => node.total_count(),
            Self::SparseLeaf(sparse) => sparse.total_count(),
            Self::DenseLeaf(dense) => dense.total_count(),
        }
    }

    fn min_key(&self, my_addr: u64, my_level: Level) -> Option<u64> {
        match self {
            Self::BranchNode(node) => node.min_key(my_addr, my_level),
            Self::SparseLeaf(sparse) => sparse.min_key(),
            Self::DenseLeaf(dense) => dense.min_key(my_addr),
        }
    }

    fn max_key(&self, my_addr: u64, my_level: Level) -> Option<u64> {
        match self {
            Self::BranchNode(node) => node.max_key(my_addr, my_level),
            Self::SparseLeaf(sparse) => sparse.max_key(),
            Self::DenseLeaf(dense) => dense.max_key(my_addr),
        }
    }

    fn range_count(&self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        match self {
            Self::BranchNode(node) => node.range_count(my_addr, my_level, range),
            Self::SparseLeaf(sparse) => sparse.range_count(range),
            Self::DenseLeaf(dense) => dense.range_count(my_addr, range),
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

    /// Returns how much the total count decreased by.
    #[must_use]
    fn decrement(&mut self, level: Level, addr: u64, dec: u32) -> u32 {
        debug_assert!(level != BOTTOM_LEVEL);
        let child_level = level - LEVEL_STEP;
        let top_addr = (addr >> level) & ADDR_MASK;
        if let Some(child) = &mut self.children[top_addr as usize] {
            let count_loss = child.decrement(child_level, addr, dec);
            if child.is_empty() {
                self.children[top_addr as usize] = None;
            }
            self.total_count -= count_loss as u64;
            count_loss
        } else {
            0
        }
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
            if range.intersects(child_range)
                && let Some(child) = &mut self.children[ci as usize]
            {
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

        self.total_count -= count_loss;

        count_loss
    }

    fn is_empty(&self) -> bool {
        self.total_count == 0
    }

    fn total_count(&self) -> u64 {
        self.total_count
    }

    fn min_key(&self, my_addr: u64, my_level: Level) -> Option<u64> {
        debug_assert!(my_level != BOTTOM_LEVEL);

        let (child_level, child_size) = child_level_and_size(my_level);

        for ci in 0..NUM_CHILDREN_IN_NODE {
            let child_addr = my_addr + ci * child_size;
            if let Some(child) = &self.children[ci as usize]
                && let Some(min_key) = child.min_key(child_addr, child_level)
            {
                return Some(min_key);
            }
        }
        None
    }

    fn max_key(&self, my_addr: u64, my_level: Level) -> Option<u64> {
        debug_assert!(my_level != BOTTOM_LEVEL);

        let (child_level, child_size) = child_level_and_size(my_level);

        for ci in (0..NUM_CHILDREN_IN_NODE).rev() {
            let child_addr = my_addr + ci * child_size;
            if let Some(child) = &self.children[ci as usize]
                && let Some(max_key) = child.max_key(child_addr, child_level)
            {
                return Some(max_key);
            }
        }
        None
    }

    fn range_count(&self, my_addr: u64, my_level: Level, range: RangeU64) -> u64 {
        debug_assert!(range.min <= range.max);
        debug_assert!(my_level != BOTTOM_LEVEL);

        let (child_level, child_size) = child_level_and_size(my_level);

        let mut total_count = 0;

        for ci in 0..NUM_CHILDREN_IN_NODE {
            let child_addr = my_addr + ci * child_size;
            let child_range = RangeU64::new(child_addr, child_addr + (child_size - 1));
            if range.intersects(child_range)
                && let Some(child) = &self.children[ci as usize]
            {
                if range.contains_all_of(child_range) {
                    total_count += child.total_count();
                } else {
                    total_count += child.range_count(child_addr, child_level, range);
                }
            }
        }

        total_count
    }
}

impl SparseLeaf {
    #[must_use]
    fn increment(mut self, level: Level, abs_addr: u64, inc: u32) -> Node {
        let index = self.addrs.partition_point(|&addr| addr < abs_addr);

        if let (Some(addr), Some(count)) = (self.addrs.get_mut(index), self.counts.get_mut(index))
            && *addr == abs_addr
        {
            *count += inc;
            return Node::SparseLeaf(self);
        }

        if self.addrs.len() < MAX_SPARSE_LEAF_LEN {
            self.addrs.insert(index, abs_addr);
            self.counts.insert(index, inc);
            Node::SparseLeaf(self)
        } else {
            // Overflow:
            let mut node = self.into_branch_node(level);
            node.increment(level, abs_addr, inc);
            Node::BranchNode(node)
        }
    }

    /// Called on overflow
    #[must_use]
    fn into_branch_node(self, level: Level) -> BranchNode {
        debug_assert!(level != BOTTOM_LEVEL);

        let mut node = BranchNode::default();
        for (key, count) in self.addrs.iter().zip(&self.counts) {
            node.increment(level, *key, *count);
        }
        node
    }

    /// Returns how much the total count decreased by.
    #[must_use]
    fn decrement(&mut self, abs_addr: u64, dec: u32) -> u32 {
        debug_assert_eq!(self.addrs.len(), self.counts.len());

        let index = self.addrs.partition_point(|&addr| addr < abs_addr);

        if let (Some(addr), Some(count)) = (self.addrs.get_mut(index), self.counts.get_mut(index))
            && *addr == abs_addr
        {
            return if dec < *count {
                *count -= dec;
                dec
            } else {
                let count_loss = *count;

                // The bucket is now empty - remove it:
                self.addrs.remove(index);
                self.counts.remove(index);
                debug_assert_eq!(self.addrs.len(), self.counts.len());

                count_loss
            };
        }

        0 // not found
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

    fn is_empty(&self) -> bool {
        self.addrs.is_empty() // we don't allow zero-sized buckets
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    fn min_key(&self) -> Option<u64> {
        self.addrs.first().copied()
    }

    fn max_key(&self) -> Option<u64> {
        self.addrs.last().copied()
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

impl DenseLeaf {
    fn increment(&mut self, abs_addr: u64, inc: u32) {
        self.counts[(abs_addr & (NUM_CHILDREN_IN_DENSE - 1)) as usize] += inc;
    }

    /// Returns how much the total count decreased by.
    #[must_use]
    fn decrement(&mut self, abs_addr: u64, dec: u32) -> u32 {
        let bucket_index = (abs_addr & (NUM_CHILDREN_IN_DENSE - 1)) as usize;
        let bucket = &mut self.counts[bucket_index];
        if dec < *bucket {
            *bucket -= dec;
            dec
        } else {
            let count_loss = *bucket;
            *bucket = 0;
            count_loss
        }
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

    fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    fn total_count(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }

    fn min_key(&self, my_addr: u64) -> Option<u64> {
        for (i, count) in self.counts.iter().enumerate() {
            if *count > 0 {
                return Some(my_addr + i as u64);
            }
        }
        None
    }

    fn max_key(&self, my_addr: u64) -> Option<u64> {
        for (i, count) in self.counts.iter().enumerate().rev() {
            if *count > 0 {
                return Some(my_addr + i as u64);
            }
        }
        None
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

impl Iterator for TreeIterator<'_> {
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
                        if self.range.intersects(child_range)
                            && let Some(Some(child)) = node.children.get(it.index)
                        {
                            it.index += 1;

                            if child_size <= self.cutoff_size
                                && self.range.contains_all_of(child_range)
                            {
                                // We can return the whole child, but first find a tight range of it:
                                if let (Some(min_key), Some(max_key)) = (
                                    child.min_key(child_addr, child_level),
                                    child.max_key(child_addr, child_level),
                                ) {
                                    return Some((
                                        RangeU64::new(min_key, max_key),
                                        child.total_count(),
                                    ));
                                } else {
                                    unreachable!("A `BranchNode` can only have non-empty children");
                                }
                            }

                            self.stack.push(NodeIterator {
                                level: child_level,
                                abs_addr: child_addr,
                                node: child,
                                index: 0,
                            });
                            continue 'outer;
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
// SizeBytes implementation

impl re_byte_size::SizeBytes for Int64Histogram {
    fn heap_size_bytes(&self) -> u64 {
        self.root.heap_size_bytes()
    }
}

impl re_byte_size::SizeBytes for Node {
    fn heap_size_bytes(&self) -> u64 {
        match self {
            Self::BranchNode(node) => node.heap_size_bytes(),
            Self::SparseLeaf(sparse) => sparse.heap_size_bytes(),
            Self::DenseLeaf(dense) => dense.heap_size_bytes(),
        }
    }
}

impl re_byte_size::SizeBytes for BranchNode {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            total_count: _,
            children,
        } = self;

        let mut total = 0;
        for child in children.iter().flatten() {
            total += child.as_ref().total_size_bytes();
        }
        total
    }
}

impl re_byte_size::SizeBytes for SparseLeaf {
    fn heap_size_bytes(&self) -> u64 {
        let Self { addrs, counts } = self;

        // SmallVec has heap data when it exceeds inline capacity
        addrs.heap_size_bytes() + counts.heap_size_bytes()
    }
}

impl re_byte_size::SizeBytes for DenseLeaf {
    fn heap_size_bytes(&self) -> u64 {
        0 // DenseLeaf is a fixed-size array on the stack
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![expect(clippy::cast_possible_wrap)] // ok in tests

    use super::*;

    #[test]
    fn test_dense() {
        let mut set = Int64Histogram::default();
        debug_assert_eq!(set.min_key(), None);
        debug_assert_eq!(set.max_key(), None);
        let mut expected_ranges = vec![];
        for i in 0..100 {
            debug_assert_eq!(set.total_count(), i);
            debug_assert_eq!(set.range_count(-10000..10000), i);
            let key = i as i64;
            set.increment(key, 1);

            expected_ranges.push((RangeI64::single(key), 1));

            debug_assert_eq!(set.min_key(), Some(0));
            debug_assert_eq!(set.max_key(), Some(key));
        }

        assert_eq!(set.range(.., 1).collect::<Vec<_>>(), expected_ranges);
        assert_eq!(set.range(..10, 1).count(), 10);

        assert_eq!(set.decrement(5, 1), 1);
        assert_eq!(set.range(..10, 1).count(), 9);
        assert_eq!(set.decrement(5, 1), 0);
        assert_eq!(set.range(..10, 1).count(), 9);
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

            debug_assert_eq!(set.min_key(), Some(0));
            debug_assert_eq!(set.max_key(), Some(key));
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

            debug_assert_eq!(set.min_key(), Some(0));
            debug_assert_eq!(set.max_key(), Some(20_000 + i));
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

    /// adjacent ranges closer than the given cutoff are treated as one
    fn glue_adjacent_ranges(ranges: &[(RangeI64, u64)], cutoff_size: u64) -> Vec<(RangeI64, u64)> {
        if ranges.is_empty() {
            return vec![];
        }

        let mut it = ranges.iter();
        let mut result = vec![*it.next().unwrap()];
        for &(new_range, new_count) in it {
            let (last_range, last_count) = result.last_mut().unwrap();
            if new_range.min.abs_diff(last_range.max) < cutoff_size {
                *last_count += new_count;
                last_range.max = new_range.max;
            } else {
                result.push((new_range, new_count));
            }
        }
        result
    }

    #[test]
    fn test_ranges_are_tight() {
        let mut set = Int64Histogram::default();
        for i in 1..=99 {
            set.increment(10_000_000 + i * 1000, 1);
            set.increment(500_000_000 + i * 1000, 1);
            set.increment(9_000_000_000 + i * 1000, 1);
        }

        let cutoff_size = 100_000;
        let ranges = set.range(.., cutoff_size).collect::<Vec<_>>();
        assert!(ranges.len() <= 10, "We shouldn't get too many ranges");

        // The Int64Histogram is allowed to split tight ranges
        //  if they hit a binary split-line, so we do a pass where we glue
        // adjacent ranges together.
        let ranges = glue_adjacent_ranges(&ranges, cutoff_size);

        assert_eq!(
            ranges,
            vec![
                (RangeI64::new(10_001_000, 10_099_000), 99),
                (RangeI64::new(500_001_000, 500_099_000), 99),
                (RangeI64::new(9_000_001_000, 9_000_099_000), 99),
            ]
        );
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

        debug_assert_eq!(set.min_key(), Some(i64::MIN));
        debug_assert_eq!(set.max_key(), Some(i64::MAX));

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

        debug_assert_eq!(set.min_key(), Some(i64::MIN));
        debug_assert_eq!(set.max_key(), Some(i64::MAX - 1));

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

        debug_assert_eq!(set.min_key(), Some(i64::MAX - 1));
        debug_assert_eq!(set.max_key(), Some(i64::MAX - 1));

        debug_assert_eq!(
            set.range(.., 1).collect::<Vec<_>>(),
            vec![(RangeI64::single(i64::MAX - 1), 2),]
        );
    }

    #[test]
    fn test_decrement() {
        let mut set = Int64Histogram::default();

        for i in 0..100 {
            set.increment(i, 2);
        }

        assert_eq!((set.min_key(), set.max_key()), (Some(0), Some(99)));
        assert_eq!(set.range(.., 1).count(), 100);

        for i in 0..100 {
            assert_eq!(set.decrement(i, 1), 1);
        }

        assert_eq!((set.min_key(), set.max_key()), (Some(0), Some(99)));
        assert_eq!(set.range(.., 1).count(), 100);

        for i in 0..50 {
            assert_eq!(set.decrement(i, 1), 1);
        }

        assert_eq!((set.min_key(), set.max_key()), (Some(50), Some(99)));
        assert_eq!(set.range(.., 1).count(), 50);

        for i in 0..50 {
            assert_eq!(
                set.decrement(i, 1),
                0,
                "Should already have been decremented"
            );
        }

        assert_eq!((set.min_key(), set.max_key()), (Some(50), Some(99)));
        assert_eq!(set.range(.., 1).count(), 50);

        for i in 50..99 {
            assert_eq!(set.decrement(i, 1), 1);
        }

        assert_eq!((set.min_key(), set.max_key()), (Some(99), Some(99)));
        assert_eq!(set.range(.., 1).count(), 1);

        assert_eq!(set.decrement(99, 1), 1);

        assert_eq!((set.min_key(), set.max_key()), (None, None));
        assert_eq!(set.range(.., 1).count(), 0);
    }

    #[test]
    fn test_next_key_after() {
        let mut hist = Int64Histogram::default();

        // Empty histogram
        assert_eq!(hist.next_key_after(0), None);

        // Single key
        hist.increment(10, 1);
        assert_eq!(hist.next_key_after(5), Some(10));
        assert_eq!(hist.next_key_after(10), Some(10)); // wraps around
        assert_eq!(hist.next_key_after(15), Some(10)); // wraps around

        // Multiple keys
        hist.increment(20, 1);
        hist.increment(30, 1);
        assert_eq!(hist.next_key_after(5), Some(10));
        assert_eq!(hist.next_key_after(10), Some(20));
        assert_eq!(hist.next_key_after(15), Some(20));
        assert_eq!(hist.next_key_after(25), Some(30));
        assert_eq!(hist.next_key_after(30), Some(10)); // wraps around
        assert_eq!(hist.next_key_after(35), Some(10)); // wraps around

        // Sparse keys
        hist = Int64Histogram::default();
        hist.increment(1000, 1);
        hist.increment(2000, 1);
        hist.increment(3000, 1);
        assert_eq!(hist.next_key_after(500), Some(1000));
        assert_eq!(hist.next_key_after(1500), Some(2000));
        assert_eq!(hist.next_key_after(2500), Some(3000));
        assert_eq!(hist.next_key_after(3500), Some(1000)); // wraps around
    }

    #[test]
    fn test_prev_key_before() {
        let mut hist = Int64Histogram::default();

        // Empty histogram
        assert_eq!(hist.prev_key_before(0), None);

        // Single key
        hist.increment(10, 1);
        assert_eq!(hist.prev_key_before(15), Some(10));
        assert_eq!(hist.prev_key_before(10), Some(10)); // wraps around
        assert_eq!(hist.prev_key_before(5), Some(10)); // wraps around

        // Multiple keys
        hist.increment(20, 1);
        hist.increment(30, 1);
        assert_eq!(hist.prev_key_before(35), Some(30));
        assert_eq!(hist.prev_key_before(30), Some(20));
        assert_eq!(hist.prev_key_before(25), Some(20));
        assert_eq!(hist.prev_key_before(15), Some(10));
        assert_eq!(hist.prev_key_before(10), Some(30)); // wraps around
        assert_eq!(hist.prev_key_before(5), Some(30)); // wraps around

        // Sparse keys
        hist = Int64Histogram::default();
        hist.increment(1000, 1);
        hist.increment(2000, 1);
        hist.increment(3000, 1);
        assert_eq!(hist.prev_key_before(3500), Some(3000));
        assert_eq!(hist.prev_key_before(2500), Some(2000));
        assert_eq!(hist.prev_key_before(1500), Some(1000));
        assert_eq!(hist.prev_key_before(500), Some(3000)); // wraps around

        // Fast path: max_key < time
        assert_eq!(hist.max_key(), Some(3000));
        assert_eq!(hist.prev_key_before(5000), Some(3000));

        // Dense histogram with many keys (tests optimization)
        hist = Int64Histogram::default();
        for i in 0..1000 {
            hist.increment(i, 1);
        }
        assert_eq!(hist.prev_key_before(500), Some(499));
        assert_eq!(hist.prev_key_before(1000), Some(999));
        assert_eq!(hist.prev_key_before(0), Some(999)); // wraps around
    }
}
