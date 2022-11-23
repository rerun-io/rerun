use std::{hash::Hash, sync::Arc};

use backtrace::Backtrace;

use crate::CountAndSize;

// ----------------------------------------------------------------------------

/// A hash of a pointer address.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct PtrHash(u64);

impl nohash_hasher::IsEnabled for PtrHash {}

impl PtrHash {
    #[inline]
    pub fn new(ptr: *mut u8) -> Self {
        let hash = ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(ptr);
        Self(hash)
    }
}

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
    let start_pattern = "re_memory::accounting_allocator::note_alloc\n";
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

    /// If this was stochastically sampled - at what rate?
    pub stochastic_rate: usize,

    /// Live allocations at this callstack.
    ///
    /// You should multiply this by [`Self::stochastic_rate`] to get an estimate
    /// of the real data.
    pub extant: CountAndSize,
}

// ----------------------------------------------------------------------------

/// Track the callstacks of allocations.
pub struct AllocationTracker {
    /// Sample every N allocations. Must be power-of-two.
    stochastic_rate: usize,

    /// De-duplicated readable backtraces.
    readable_backtraces: nohash_hasher::IntMap<BacktraceHash, ReadableBacktrace>,

    /// Current live allocations.
    live_allocs: ahash::HashMap<PtrHash, BacktraceHash>,

    /// How much memory is allocated by each callstack?
    callstack_stats: nohash_hasher::IntMap<BacktraceHash, CountAndSize>,
}

impl AllocationTracker {
    pub fn with_stochastic_rate(stochastic_rate: usize) -> Self {
        assert!(stochastic_rate != 0);
        assert!(stochastic_rate.is_power_of_two());
        Self {
            stochastic_rate,
            readable_backtraces: Default::default(),
            live_allocs: Default::default(),
            callstack_stats: Default::default(),
        }
    }

    fn should_sample(&self, ptr: PtrHash) -> bool {
        ptr.0 & (self.stochastic_rate as u64 - 1) == 0
    }

    pub fn on_alloc(&mut self, ptr: PtrHash, size: usize) {
        if !self.should_sample(ptr) {
            return;
        }

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

    pub fn on_dealloc(&mut self, ptr: PtrHash, size: usize) {
        if !self.should_sample(ptr) {
            return;
        }

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

    /// Return the `n` callstacks that currently is using the most memory.
    pub fn top_callstacks(&self, n: usize) -> Vec<CallstackStatistics> {
        let mut vec: Vec<_> = self
            .callstack_stats
            .iter()
            .filter(|(_hash, c)| c.count > 0)
            .filter_map(|(hash, c)| {
                Some(CallstackStatistics {
                    readable_backtrace: self.readable_backtraces.get(hash)?.clone(),
                    stochastic_rate: self.stochastic_rate,
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
