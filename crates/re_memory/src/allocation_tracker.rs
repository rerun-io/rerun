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
///
/// Clones without allocating.
#[derive(Clone)]
pub struct ReadableBacktrace {
    /// Human-readable backtrace.
    readable: Arc<str>,
}

impl std::fmt::Display for ReadableBacktrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.readable.fmt(f)
    }
}

impl ReadableBacktrace {
    fn new(mut backtrace: Backtrace) -> Self {
        backtrace.resolve();
        let readable = format_backtrace(&backtrace);
        Self { readable }
    }
}

fn format_backtrace(backtrace: &Backtrace) -> Arc<str> {
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

    stack.into()
}

// ----------------------------------------------------------------------------

/// Per-callstack statistics.
#[derive(Clone)]
pub struct CallstackStatistics {
    /// For when we print this statistic.
    pub readable_backtrace: ReadableBacktrace,

    /// Live allocations at this callstack.
    pub extant: CountAndSize,
}

// ----------------------------------------------------------------------------

/// Track the callstacks of allocations.
#[derive(Default)]
pub struct AllocationTracker {
    /// De-duplicated readable backtraces.
    readable_backtraces: nohash_hasher::IntMap<BacktraceHash, ReadableBacktrace>,

    /// Current live allocations. Key = pointer address.
    live_allocs: ahash::HashMap<usize, BacktraceHash>,

    /// How much memory is allocated by each callstack?
    callstack_stats: nohash_hasher::IntMap<BacktraceHash, CountAndSize>,
}

impl AllocationTracker {
    pub fn on_alloc(&mut self, ptr: usize, size: usize) {
        let unresolved_backtrace = Backtrace::new_unresolved();
        let hash = hash_backtrace(&unresolved_backtrace);

        self.readable_backtraces
            .entry(hash)
            .or_insert_with(|| ReadableBacktrace::new(unresolved_backtrace));

        {
            self.callstack_stats.entry(hash).or_default().add(size);
        }

        self.live_allocs.insert(ptr, hash);
    }

    pub fn on_dealloc(&mut self, ptr: usize, size: usize) {
        if let Some(hash) = self.live_allocs.remove(&ptr) {
            if let std::collections::hash_map::Entry::Occupied(mut entry) =
                self.callstack_stats.entry(hash)
            {
                let stats = entry.get_mut();
                stats.sub(size);

                // Free up some memory:
                if stats.size == 0 {
                    entry.remove();
                }
            }
        }
    }

    /// Number of bytes in the live allocations we are tracking.
    pub fn tracked_allocs_and_bytes(&self) -> CountAndSize {
        let mut count_and_size = CountAndSize::ZERO;
        for c in self.callstack_stats.values() {
            count_and_size.count += c.count;
            count_and_size.size += c.size;
        }
        count_and_size
    }

    /// Return the `n` callstacks that currently is using the most memory.
    pub fn top_callstacks(&self, n: usize) -> Vec<CallstackStatistics> {
        let mut vec: Vec<_> = self
            .callstack_stats
            .iter()
            .filter(|(_hash, c)| c.count > 0)
            .filter_map(|(hash, c)| {
                Some(CallstackStatistics {
                    readable_backtrace: self.readable_backtraces.get(hash)?.clone(),
                    extant: *c,
                })
            })
            .collect();
        vec.sort_by_key(|stats| -(stats.extant.size as i64));
        vec.truncate(n);
        vec.shrink_to_fit();
        vec
    }
}
