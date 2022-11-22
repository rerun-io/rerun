use std::{hash::Hash, sync::Arc};

use backtrace::Backtrace;

use crate::CountAndSize;

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct BacktraceHash(u64);

impl nohash_hasher::IsEnabled for BacktraceHash {}

fn hash_backtrace(backtrace: &Backtrace) -> BacktraceHash {
    use std::hash::Hasher as _;
    let mut hasher =
        std::hash::BuildHasher::build_hasher(&ahash::RandomState::with_seeds(0, 1, 2, 3));

    for frame in backtrace.frames() {
        frame.ip().hash(&mut hasher);
    }

    BacktraceHash(hasher.finish())
}

// ----------------------------------------------------------------------------

/// Formatted [`Backtrace`].
pub struct ReadableBacktrace {
    hash: BacktraceHash,

    /// Human-readable backtrace.
    readable: String,
}

impl std::fmt::Display for ReadableBacktrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.readable.fmt(f)
    }
}

impl ReadableBacktrace {
    fn new(hash: BacktraceHash, mut backtrace: Backtrace) -> Self {
        backtrace.resolve();
        let readable = format_backtrace(&backtrace);
        Self { hash, readable }
    }
}

fn format_backtrace(backtrace: &Backtrace) -> String {
    let stack = format!("{:?}", backtrace);
    let mut stack = stack.as_str();
    let start_pattern = "<re_memory::tracking_allocator::TrackingAllocator<InnerAllocator> as core::alloc::global::GlobalAlloc>::alloc\n";
    if let Some(start_offset) = stack.find(start_pattern) {
        stack = &stack[start_offset + start_pattern.len()..];
    }

    if let Some(end_offset) = stack.find("std::sys_common::backtrace::__rust_begin_short_backtrace")
    {
        stack = &stack[..end_offset];
    }

    stack.to_owned()
}

// ----------------------------------------------------------------------------

/// Per-callstack statistics.
#[derive(Clone)]
pub struct CallstackStatistics {
    /// For when we print this statistic.
    pub readable_backtrace: Arc<ReadableBacktrace>,

    /// Live allocations at this callstack.
    pub extant: CountAndSize,
}

// ----------------------------------------------------------------------------

/// Track the callstacks of allocations.
#[derive(Default)]
pub struct AllocationTracker {
    /// De-duplicated readable backtraces.
    readable_backtraces: nohash_hasher::IntMap<BacktraceHash, Arc<ReadableBacktrace>>,

    /// Current live allocations. Key = pointer address.
    live_allocs: ahash::HashMap<usize, BacktraceHash>,

    /// How much memory is allocated by each callstack?
    callstacks: nohash_hasher::IntMap<BacktraceHash, CallstackStatistics>,
}

impl AllocationTracker {
    pub fn on_alloc(&mut self, ptr: usize, size: usize) {
        let unresolved_backtrace = Backtrace::new_unresolved();
        let hash = hash_backtrace(&unresolved_backtrace);

        let readable_backtrace = self
            .readable_backtraces
            .entry(hash)
            .or_insert_with(|| Arc::new(ReadableBacktrace::new(hash, unresolved_backtrace)))
            .clone();

        {
            let mut stats = self
                .callstacks
                .entry(readable_backtrace.hash)
                .or_insert_with(|| CallstackStatistics {
                    readable_backtrace: readable_backtrace.clone(),
                    extant: CountAndSize::ZERO,
                });
            stats.extant.count += 1;
            stats.extant.size += size;
        }

        self.live_allocs.insert(ptr, hash);
    }

    pub fn on_dealloc(&mut self, ptr: usize, size: usize) {
        if let Some(hash) = self.live_allocs.remove(&ptr) {
            if let std::collections::hash_map::Entry::Occupied(mut entry) =
                self.callstacks.entry(hash)
            {
                let stats = entry.get_mut();
                stats.extant.size -= size;
                stats.extant.count -= 1;

                // Free up some memory:
                if stats.extant.size == 0 {
                    entry.remove();
                }
            }
        }
    }

    /// Number of bytes in the live allocations we are tracking.
    pub fn tracked_allocs_and_bytes(&self) -> CountAndSize {
        let mut count_and_size = CountAndSize::ZERO;
        for c in self.callstacks.values() {
            count_and_size.count += c.extant.count;
            count_and_size.size += c.extant.size;
        }
        count_and_size
    }

    /// Return the `n` callstacks that currently is using the most memory.
    pub fn top_callstacks(&self, n: usize) -> Vec<&CallstackStatistics> {
        if true {
            // Simple and fast enough
            let mut vec: Vec<_> = self
                .callstacks
                .values()
                .filter(|c| c.extant.count > 0)
                .collect();
            vec.sort_by_key(|tracked| -(tracked.extant.size as i64));
            vec.truncate(n);
            vec.shrink_to_fit();
            vec
        } else {
            // Fast
            struct SmallestSize<'a>(&'a CallstackStatistics);
            impl<'a> PartialEq for SmallestSize<'a> {
                fn eq(&self, other: &Self) -> bool {
                    self.0.extant.size == other.0.extant.size
                }
            }
            impl<'a> Eq for SmallestSize<'a> {}
            impl<'a> PartialOrd for SmallestSize<'a> {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }
            impl<'a> Ord for SmallestSize<'a> {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.0.extant.size.cmp(&other.0.extant.size).reverse()
                }
            }

            let mut binary_heap =
                std::collections::BinaryHeap::<SmallestSize<'_>>::with_capacity(n);

            for candidate in self.callstacks.values() {
                if candidate.extant.count > 0 {
                    if let Some(SmallestSize(top)) = binary_heap.peek() {
                        if candidate.extant.size > top.extant.size {
                            if binary_heap.len() > n {
                                binary_heap.pop();
                            }
                            binary_heap.push(SmallestSize(candidate));
                        }
                    } else {
                        binary_heap.push(SmallestSize(candidate));
                    }
                }
            }

            let mut vec: Vec<_> = binary_heap
                .drain()
                .map(|SmallestSize(tracked)| tracked)
                .collect();
            vec.sort_by_key(|tracked| tracked.extant.size);
            vec.reverse();
            vec
        }
    }
}
