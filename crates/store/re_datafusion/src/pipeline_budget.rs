//! Adaptive memory budget for the chunk IO pipeline.
//!
//! # Overview
//!
//! When querying a dataset, chunks are fetched from remote storage (S3 / gRPC)
//! and decoded into Arrow before being inserted into a [`ChunkStore`] for query
//! execution. Without backpressure the IO pipeline would fetch as fast as the
//! network allows, potentially consuming unbounded memory before the CPU thread
//! can process and GC the data.
//!
//! [`PipelineBudget`] solves this by tracking the total decoded bytes currently
//! in-flight and blocking IO tasks when the budget is exhausted. This creates a
//! natural sliding-window: the IO side stays ahead of the CPU side by at most
//! `budget` bytes, preventing OOM while keeping the pipeline saturated.
//!
//! # Budget sizing
//!
//! `BUDGET_FRACTION` is the share of a query's total decoded data that the
//! pipeline is allowed to hold in RAM at once — i.e. the size of the sliding
//! window the IO side may run ahead of the CPU side.
//!
//! The budget is adaptive — derived from the total uncompressed chunk sizes
//! reported by the server in the `QueryDatasetResponse`:
//!
//! ```text
//! per_partition = (total_uncompressed * BUDGET_FRACTION) / num_partitions
//! per_partition = clamp(per_partition, MIN_BUDGET_PER_PARTITION, MAX_BUDGET_PER_PARTITION)
//! budget       = per_partition * num_partitions
//! ```
//!
//! ## Compile-time defaults: budget is currently effectively disengaged
//!
//! The shipping defaults (`FRACTION=1.0`, `MIN=4 GiB`, `MAX=1 TiB`) are
//! picked so the budget never bites in practice. The current CPU worker
//! buffers an entire segment before releasing, so a per-partition cap
//! below the largest decoded segment's working set deadlocks: chunks
//! pin the budget at full before the segment-finalization `release`
//! fires, with no path to forward progress. Reproduced on PR #1736
//! against `rerun-synthetic-structs-10k` at 50 segments — adaptive
//! sizing produced a 283 MB total budget that pinned at 282/283 with
//! 72 IO tasks parked at wait #1 and zero `release` calls.
//!
//! Once the follow-up CPU-worker streaming-release refactor lands —
//! which releases chunks as the safe time horizon advances rather than
//! at segment boundaries — dial these back to realistic host RSS
//! budgets (originally targeted: `FRACTION=0.25`, `MIN=64 MiB`, `MAX=1 GiB`).
//! With those values, for a 4 GB query at most ~1 GB of decoded chunks
//! live in RAM while the remaining ~3 GB are still on the wire, on disk,
//! or already flushed downstream — small queries still stream (no reason
//! to buffer everything when the working set is tiny) and large queries
//! stay well clear of the host RSS limit. Tighter fractions trade memory
//! headroom for more frequent IO stalls; looser fractions risk OOM under
//! co-tenancy.
//!
//! When the server does not provide uncompressed sizes (older server), the
//! compressed wire size is used as a fallback — this under-estimates, producing
//! more backpressure rather than less.
//!
//! ## Runtime overrides
//!
//! The three sizing parameters can be tuned without a rebuild via
//! environment variables:
//!
//! | Variable                          | Type   | Accepted range | Default |
//! |-----------------------------------|--------|----------------|---------|
//! | `RERUN_PIPELINE_BUDGET_MIN`       | size   | `> 0`          | `4GiB`  |
//! | `RERUN_PIPELINE_BUDGET_MAX`       | size   | `> 0`          | `1TiB`  |
//! | `RERUN_PIPELINE_BUDGET_FRACTION`  | float  | `(0.0, 1.0]`   | 1.0     |
//!
//! Sizes accept either a SI/IEC suffix (`64MB`, `1GiB`, `512KiB`) or a
//! bare positive integer interpreted as bytes. Values are trimmed of
//! surrounding whitespace; empty strings are treated as unset. Unparsable
//! or out-of-range values are logged at error level and the affected
//! parameter falls back to its compile-time default. If `MIN > MAX`
//! after overrides, both revert to defaults (rather than panicking on
//! the downstream `clamp()`).
//!
//! # Adaptive estimation
//!
//! Sizing the budget from `total_uncompressed` is only half the story —
//! individual reservations also need a sensible per-fetch size. The
//! server-reported uncompressed chunk size is a wire-encoding estimate;
//! the actual decoded `SizeBytes` (Arrow heap + dictionary + index
//! overhead) can drift above or below that. To keep reservations honest,
//! [`PipelineBudget`] maintains a learned multiplier:
//!
//! ```text
//! reserved = estimated_uncompressed * estimate_multiplier
//! ```
//!
//! Each completed fetch feeds an `(estimated, actual)` sample back via
//! [`PipelineBudget::adjust_reservation`]; the raw ratio is clamped to
//! `[MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER]` (so a single
//! pathological chunk can't pin every future reservation at the ceiling)
//! and folded into an EMA with smoothing factor [`ESTIMATE_EMA_ALPHA`]
//! (α=0.2 — converges within a handful of samples while staying tolerant
//! of one-off outliers). The multiplier starts at
//! [`INITIAL_ESTIMATE_MULTIPLIER`] (1.5x) so the first few cold-start
//! reservations over-account rather than under-account, then settles to
//! the dataset's true ratio (typically near 1.0 for the workloads
//! measured in PR #1736).
//!
//! # Protocol
//!
//! 1. **Before fetch:** IO task calls [`PipelineBudget::reserve_with_priority`]
//!    with the chunk's uncompressed size, a priority key (`task_time_min`,
//!    earliest-first), and the distinct `segment_ids` the fetch touches.
//!    Admission is gated on both available bytes *and* the segment-count gate
//!    ([`MAX_CONCURRENT_SEGMENTS`]). If either is full, the call blocks. A
//!    parked reserver wakes when any of:
//!    - [`PipelineBudget::release`] runs on the CPU thread,
//!    - [`PipelineBudget::publish_segment_finalized`] frees a segment slot,
//!    - [`PipelineBudget::adjust_reservation`] shrinks an earlier reservation,
//!    - another `reserve_with_priority` succeeds with remaining headroom and
//!      cascade-wakes the next admittable waiter.
//! 2. **After fetch:** IO task calls [`PipelineBudget::adjust_reservation`] to
//!    correct the estimate to the actual decoded Arrow heap size.
//! 3. **After segment finalization:** CPU thread calls
//!    [`PipelineBudget::release`] to return the segment's freed bytes and
//!    [`PipelineBudget::publish_segment_finalized`] to vacate its slot in the
//!    segment-count gate. Both wake blocked IO tasks; the two are independent
//!    (bytes may be refunded incrementally, the slot only at finalization).
//!
//! # Exhaustion behavior
//!
//! When `current >= budget` and a new [`PipelineBudget::reserve_with_priority`] arrives, the
//! pipeline transitions from network-rate-limited to release-rate-limited
//! throughput. The sequence:
//!
//! 1. **Single-fetch bypass.** If `reserved_bytes > budget` (one chunk is
//!    larger than the entire shared budget), the reservation is admitted
//!    unconditionally with a warn-level log. The budget temporarily
//!    over-commits rather than deadlocking on a fetch that could never fit.
//!    This case is not expected in practice — typical chunks are orders of
//!    magnitude below the per-partition cap.
//! 2. **Fast path.** `try_acquire` attempts to claim the reservation
//!    atomically via a compare-and-swap (CAS) loop on `current`: read the
//!    current value, refuse to commit if `current + reserved_bytes >
//!    budget`, otherwise atomically advance only when nothing else has
//!    moved `current` since the read. On contention only the
//!    actually-committed reservations move `current`, so concurrent
//!    reservers don't transiently observe an over-budget value and bail
//!    out spuriously (the "thundering herd" pattern of a naive
//!    `fetch_add → check → fetch_sub` design).
//! 3. **Park.** On failure the task allocates a [`tokio::sync::Notify`],
//!    pushes it onto the priority `wait_queue` (a `BinaryHeap` keyed by
//!    `task_time_min`, earliest-first), then **rechecks** `try_admit` once
//!    more before awaiting. The recheck closes a lost-wakeup race: if a
//!    [`PipelineBudget::release`] / [`PipelineBudget::adjust_reservation`] /
//!    [`PipelineBudget::publish_segment_finalized`] fires between the initial
//!    fast-path miss and the enqueue, the recheck observes the freed budget
//!    and the task proceeds without ever awaiting.
//!    The first park and every tenth re-park emit an info-level backpressure
//!    log so the condition is visible in production without needing
//!    `RUST_LOG=debug`.
//!
//! Parked reservers wake from any of four sources:
//!
//! - [`PipelineBudget::release`] (CPU thread, as a segment's bytes free up) —
//!   the dominant source under steady-state load.
//! - [`PipelineBudget::publish_segment_finalized`] (CPU thread, at segment
//!   finalization) — vacates a segment-count-gate slot for a waiter that was
//!   blocked on the gate rather than on bytes.
//! - [`PipelineBudget::adjust_reservation`] when `actual < reserved` (IO
//!   thread, on fetch completion that under-ran its reservation) — refunds
//!   the sliver between the multiplier-scaled estimate and the measured
//!   decoded size.
//! - Cascade-wake from a sibling `reserve` that succeeded with remaining
//!   headroom, ensuring a chain of small reservations doesn't strand a
//!   single large waiter at the head of the queue.
//!
//! Wake-up order is by priority: [`PipelineBudget::wake_next`] pops the
//! lowest-`task_time_min` *admittable* waiter from the `BinaryHeap`, skipping
//! cancelled entries and waiters still blocked on the segment-count gate so a
//! freed budget isn't wasted on a higher-priority waiter that can't yet use
//! it. A woken task re-enters the acquire loop from the top: a successful
//! claim returns; a still-insufficient claim re-enqueues a fresh `Notify` and
//! re-parks.
//!
//! While parked, the IO task holds its slot in the upstream
//! `buffer_unordered` stage, so backpressure naturally propagates: the
//! number of in-flight fetches drops to whatever fits the current `budget -
//! current` headroom. Network bandwidth is left idle by design — that is
//! the entire point of the budget.
//!
//! Steady-state throughput under exhaustion equals the CPU worker's
//! release rate (segments finalized per unit time × bytes per segment),
//! independent of network capacity. Diagnostics for tuning live on the
//! struct itself: `peak_current` is atomically max'd on every commit,
//! `total_released_bytes` and `total_releases` are accumulated, and
//! [`PipelineBudget`]'s `Drop` impl emits a lifecycle summary log
//! suitable for post-hoc query analysis.
//!
//! # Edge cases
//!
//! - A single chunk larger than the entire budget is allowed through with a
//!   warning to avoid deadlock. The budget temporarily goes over-committed and
//!   recovers after release.
//! - The learned `estimate_multiplier` is per-`PipelineBudget` and a fresh
//!   budget is constructed for every query, so each query starts at
//!   [`INITIAL_ESTIMATE_MULTIPLIER`] regardless of what previous queries
//!   on the same process learned. The first few fetches in a cold-start
//!   query therefore over-reserve by the bootstrap factor (default 1.5x);
//!   the EMA pulls the multiplier back down within a handful of samples.
//!   Cross-query persistence is intentionally not implemented — different
//!   schemas/codecs decode at different ratios, and a stale multiplier
//!   from another query would be worse than a brief cold start.
//! - All atomic operations use `AcqRel`/`Acquire` ordering to guarantee
//!   cross-thread visibility on weakly-ordered architectures (ARM).
//!
//! [`ChunkStore`]: re_dataframe::external::re_chunk_store::ChunkStore

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool, AtomicU32, AtomicU64, AtomicUsize,
    Ordering::{AcqRel, Acquire, Relaxed, Release},
};

use re_log_types::TimeInt;

use parking_lot::Mutex;
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// Defaults are intentionally tuned to leave the budget effectively disengaged.
//
// The current CPU worker buffers an entire segment before releasing, so any
// per-partition cap below the largest decoded segment's working set risks
// deadlock: a segment's chunks pin the budget at full before that segment's
// release fires at finalization, with no way to make forward progress.
// Empirically reproduced on PR #1736 against `rerun-synthetic-structs-10k`
// at 50 segments — adaptive sizing produced a 283 MB total budget that
// pinned at 282/283 with 72 IO tasks parked at wait #1, no `release` ever
// fired (zero `remote materialize` lines after 22 min stall).
//
// Until the follow-up CPU-worker refactor lands — which releases chunks as
// the safe time horizon advances rather than at segment boundaries — both
// FRACTION and the MIN/MAX clamps are set so the budget never bites in
// practice:
//
//   per_partition = (total * 1.0) / num_partitions
//   per_partition = clamp(per_partition, 4 GiB, 1 TiB)   // both bounds huge
//   budget        = per_partition * num_partitions       // ≥ 4 GiB * N
//
// Once streaming release is in place, dial these back to realistic host
// RSS budgets (originally targeted: FRACTION=0.25, MIN=64 MiB, MAX=1 GiB).
// ---------------------------------------------------------------------------

/// Default fraction of total query data to allow in-flight at once.
/// Override with [`ENV_BUDGET_FRACTION`]. Set to `1.0` so adaptive sizing
/// uses the full uncompressed estimate; see top-of-section note for why.
const BUDGET_FRACTION: f64 = 1.0;

/// Default minimum adaptive budget per partition.
/// Override with [`ENV_BUDGET_MIN`]. Set high so the per-partition clamp's
/// lower bound dominates and adaptive sizing can't pull the budget below
/// a single segment's working set; see top-of-section note for why.
pub(crate) const MIN_BUDGET_PER_PARTITION: usize = 4 * 1024 * 1024 * 1024; // 4 GiB

/// Default maximum adaptive budget per partition.
/// Override with [`ENV_BUDGET_MAX`]. See top-of-section note.
pub(crate) const MAX_BUDGET_PER_PARTITION: usize = 1024 * 1024 * 1024 * 1024; // 1 TiB

/// Environment variable to override the minimum per-partition budget.
/// Value accepts a SI/IEC suffix (`64MB`, `1GiB`, `512KiB`) or a bare
/// positive integer interpreted as bytes; must be `> 0`. Invalid values
/// are logged and the compile-time default is used.
const ENV_BUDGET_MIN: &str = "RERUN_PIPELINE_BUDGET_MIN";

/// Environment variable to override the maximum per-partition budget.
/// Value accepts a SI/IEC suffix (`64MB`, `1GiB`, `512KiB`) or a bare
/// positive integer interpreted as bytes; must be `> 0`. Invalid values
/// are logged and the compile-time default is used. If MIN ends up
/// greater than MAX after override, both fall back to defaults.
const ENV_BUDGET_MAX: &str = "RERUN_PIPELINE_BUDGET_MAX";

/// Environment variable to override the in-flight fraction. Value is
/// a float in `(0.0, 1.0]`. Invalid values are logged and the
/// compile-time default is used.
const ENV_BUDGET_FRACTION: &str = "RERUN_PIPELINE_BUDGET_FRACTION";

/// Bootstrap multiplier applied to `reserve` estimates before any `actual`
/// samples have been observed.
///
/// Empirically, on the synthetic small / medium / long workloads measured
/// during PR #1736 the steady-state ratio of decoded `SizeBytes` to the
/// server-reported uncompressed wire size sits at ~1.00–1.005. The
/// learned multiplier converges to that value via the EMA after a few
/// samples. We bootstrap higher (1.5x) so the first reservations of a
/// fresh budget over-account rather than under-account: a cold-start
/// query reserves ~50% more than it ends up using until the EMA pulls
/// the multiplier back down. That over-reservation is paid for by extra
/// backpressure during the first few fetches, which is preferable to a
/// transient OOM if the first chunk happens to expand more than typical.
const INITIAL_ESTIMATE_MULTIPLIER: f64 = 1.5;

/// EMA smoothing factor for the estimate→actual ratio. Low α = smooth
/// (tolerant to a one-off large chunk), high α = reactive.
const ESTIMATE_EMA_ALPHA: f64 = 0.2;

/// Floor on the learned multiplier. Never reserve *less* than the raw
/// uncompressed estimate: decoded size is always ≥ wire size for data we
/// care about here.
const MIN_ESTIMATE_MULTIPLIER: f64 = 1.0;

/// Ceiling on the learned multiplier. A single pathological chunk with
/// exceptional expansion shouldn't starve the pipeline by inflating every
/// future reservation.
const MAX_ESTIMATE_MULTIPLIER: f64 = 3.0;

/// Cap on the number of distinct segments that may be in-flight on the
/// IO side simultaneously. Independent of the byte budget — even if
/// bytes are available, only this many segments can have a parked
/// fetch reservation at once. The cap keeps the CPU worker's `HashMap`
/// of open `CurrentStores` from growing without bound under high
/// network concurrency, which directly bounds the cross-segment
/// reorder pressure a single safe-horizon advance has to resolve.
///
/// `create_request_batches` reads this constant to cap the distinct
/// segments per merged fetch batch — without that, a small-segment
/// workload would merge many segments into a single 8 MB fetch and
/// every such fetch with `new_segments > MAX_CONCURRENT_SEGMENTS`
/// would deadlock on the gate (the gate can never admit it).
pub(crate) const MAX_CONCURRENT_SEGMENTS: usize = 3;

/// Number of consecutive `flush_incremental` calls that observed zero
/// emittable rows while the budget was near-saturated before
/// [`PipelineBudget`] sets `force_overcommit`. The threshold is high
/// enough that ordinary "horizon hasn't moved yet" stalls don't trip
/// it, but low enough that a real deadlock is broken within a few
/// hundred ms on realistic workloads.
const STALL_EMPTY_EMIT_THRESHOLD: u32 = 20;

/// Fraction of the budget that must be in use before the stall
/// detector even considers tripping the breaker. Without this gate, a
/// query that genuinely has nothing to emit (waiting on slow IO with
/// the budget mostly empty) would falsely trigger.
const STALL_SATURATION_THRESHOLD: f64 = 0.95;

/// Waiter parked on the budget's wait queue. Ordered by
/// `task_time_min` ascending (earliest first), tie-broken by `seq`
/// (enqueue order) so two reservers for the same `task_time_min`
/// preserve FIFO semantics.
///
/// Priority is meaningful only for queries with a temporal
/// `filtered_index`: when the CPU worker is waiting for the
/// horizon-advancing chunk to clear the budget, that chunk's IO task
/// has the lowest `task_time_min` and should be woken first. Static
/// queries pass `TimeInt::MAX` to put their waiters at the back of
/// the heap.
struct PriorityWaiter {
    task_time_min: TimeInt,
    seq: u64,
    notify: Arc<Notify>,

    /// Set to `true` by the reserver when the second `try_admit`
    /// (post-enqueue race-recovery) succeeds and the reserver returns
    /// without ever awaiting `notify`. [`PipelineBudget::wake_next`]
    /// drains cancelled entries on its way to a real waiter so the
    /// "lowest-`task_time_min` orphan sits at the top of the heap and
    /// captures every wake" failure mode of the priority queue does
    /// not steal wake events from genuine parked waiters.
    cancelled: Arc<AtomicBool>,

    /// Reservation size and distinct segment set this waiter is parked
    /// for. Read by [`PipelineBudget::wake_next`] to test whether the
    /// waiter could actually be admitted right now, so a freed budget
    /// is handed to a waiter that can use it rather than wasted on a
    /// higher-priority waiter still blocked on the segment-count gate.
    reserved_bytes: usize,
    segment_ids: Vec<String>,
}

impl PartialEq for PriorityWaiter {
    fn eq(&self, other: &Self) -> bool {
        self.task_time_min == other.task_time_min && self.seq == other.seq
    }
}

impl Eq for PriorityWaiter {}

impl PartialOrd for PriorityWaiter {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityWaiter {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.task_time_min
            .cmp(&other.task_time_min)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}

/// Tracks the segments currently holding a reservation against the
/// budget, plus the subset that was admitted past
/// [`MAX_CONCURRENT_SEGMENTS`] via the stall-breaker `force_overcommit`
/// bypass. Wrapped in a single [`Mutex`] so the cap check (which reads
/// both fields) is atomic with the membership update.
#[derive(Default)]
struct SegmentGate {
    /// Every segment that currently has at least one reservation
    /// outstanding against the byte budget.
    all: HashSet<String>,

    /// Subset of `all` that was admitted while
    /// [`PipelineBudget::force_overcommit`] was set. These segments do
    /// NOT consume a slot of [`MAX_CONCURRENT_SEGMENTS`] for future
    /// admissions — they are over-cap by design, and the cap contract
    /// is restored as soon as they finalize.
    bypass: HashSet<String>,
}

impl SegmentGate {
    /// Effective in-flight count for cap enforcement: total in-flight
    /// segments minus the bypass-admitted overflow.
    fn effective_len(&self) -> usize {
        self.all.len().saturating_sub(self.bypass.len())
    }
}

/// Tracks total decoded bytes in the pipeline and enforces a memory budget.
///
/// See the [module-level documentation](self) for the full design.
pub(crate) struct PipelineBudget {
    /// Maximum decoded bytes allowed in the pipeline at any time.
    budget: usize,

    /// Current decoded bytes in the pipeline (IO buffers + channel + `ChunkStore`).
    current: AtomicUsize,

    /// Priority queue of parked reserve-waiters. `release` /
    /// `adjust_reservation` / `publish_segment_finalized` wake the
    /// **earliest-time** waiter first so the chunk that would advance
    /// the CPU side's safe horizon preempts later-time chunks under
    /// saturation. Ties broken by enqueue order via
    /// [`PriorityWaiter::seq`].
    wait_queue: Mutex<BinaryHeap<Reverse<PriorityWaiter>>>,

    /// Monotonically incremented per `reserve` call to break
    /// `task_time_min` ties in the priority heap with FIFO semantics.
    wait_seq: AtomicU64,

    /// Set of segments that currently hold a reservation against the
    /// budget. A new `reserve` only succeeds if its full set of
    /// `segment_ids` would not push the union past
    /// [`MAX_CONCURRENT_SEGMENTS`]. Cleared on
    /// [`Self::publish_segment_finalized`] (called from
    /// `Drop for CurrentStores`) — never on `release`, because byte
    /// release happens before the CPU worker is done with the
    /// segment.
    ///
    /// `bypass` is the subset of `all` that was admitted past the cap
    /// via [`Self::force_overcommit`]. For cap accounting,
    /// `effective_len = all.len() - bypass.len()` — i.e.,
    /// bypass-admitted segments do not consume a slot of the cap. This
    /// is what restores the cap contract after the stall breaker
    /// fires: without the bypass set, post-recovery `try_admit` calls
    /// would block on the inflated `all.len()` until enough segments
    /// finalize, silently breaking the cap.
    active_segments: Mutex<SegmentGate>,

    /// Consecutive `flush_incremental` calls that emitted zero rows
    /// while the budget was near-saturated. Reset on
    /// [`Self::notify_row_emitted`] or [`Self::release`].
    empty_emit_count: AtomicU32,

    /// Stall-detector escape hatch. When set, `reserve` bypasses both
    /// the byte budget and the segment-count gate so a parked
    /// horizon-advancing fetch can break out of an out-of-order
    /// deadlock. Cleared on the next real progress signal.
    ///
    /// Deliberately a separate latch rather than derived from
    /// `empty_emit_count >= STALL_EMPTY_EMIT_THRESHOLD`: the counter is
    /// a *measurement* that [`Self::notify_empty_emit`] resets to zero
    /// on any unsaturated empty emit, while the latch must stay armed
    /// until *real progress* ([`Self::release`],
    /// [`Self::notify_row_emitted`], or a shrink in
    /// [`Self::adjust_reservation`]) disarms it. If the breaker were
    /// derived from the counter, an unsaturated empty emit between the
    /// stall-breaker wake and the woken waiter's `try_admit` recheck
    /// would disarm it mid-recovery — the waiter re-parks, the stall
    /// resumes, and the counter has to climb through the full
    /// threshold again before the next attempt.
    force_overcommit: AtomicBool,

    /// Learned multiplier applied to `reserve` estimates so reservations
    /// track true decoded size rather than the raw (often low)
    /// uncompressed estimate. Stored as `f64::to_bits` in an atomic so
    /// `reserve` can read lock-free; updated via a CAS loop in
    /// `adjust_reservation`. Starts at [`INITIAL_ESTIMATE_MULTIPLIER`]
    /// and converges via EMA toward the dataset's actual
    /// `actual / estimated` ratio, clamped to
    /// `[MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER]`.
    estimate_multiplier: AtomicU64,

    /// Highest value `current` ever reached during the lifetime of
    /// this budget. Used in the lifecycle summary emitted on `Drop`.
    peak_current: AtomicUsize,

    /// Cumulative bytes ever passed to [`Self::release`]. Lifecycle
    /// summary diagnostic only.
    total_released_bytes: AtomicUsize,

    /// Number of [`Self::release`] calls. Lifecycle summary
    /// diagnostic only.
    total_releases: AtomicU64,

    /// Test-only seam exposing the otherwise-unobservable gap inside
    /// `reserve` between enqueuing the wait notify and the recheck
    /// `try_acquire` / await. When armed, `reserve` signals
    /// [`TestPauseHook::arrived`] after pushing the notify onto the
    /// wait queue, then awaits [`TestPauseHook::resume`] before
    /// retrying. Lets tests deterministically inject a `release` /
    /// `adjust_reservation` into that gap and assert no wake-up is
    /// lost.
    #[cfg(test)]
    test_pause_hook: parking_lot::Mutex<Option<TestPauseHook>>,
}

#[cfg(test)]
#[derive(Clone)]
struct TestPauseHook {
    arrived: Arc<Notify>,
    resume: Arc<Notify>,
}

/// Read a non-empty, trimmed environment variable.
///
/// Returns `Some(trimmed)` when the variable is set to a non-empty,
/// valid-Unicode value. Returns `None` when unset, empty, or
/// whitespace-only. Logs an error and returns `None` if the value
/// is not valid Unicode.
fn read_env_trimmed(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            re_log::error!("{key}: value is not valid Unicode; using default");
            None
        }
    }
}

/// Parse a byte size from a pre-trimmed string. Accepts a SI/IEC suffix
/// (`64MB`, `1GiB`, `512KiB`) via [`re_format::parse_bytes`] or a bare
/// positive integer interpreted as bytes; falls back to `default_bytes`
/// when unparsable or ≤ 0. All failure modes produce an error-level
/// log. Pure function — no env access — so it is safe to unit-test.
fn parse_bytes_or_default(key: &str, raw: &str, default_bytes: usize) -> usize {
    let parsed = re_format::parse_bytes(raw).or_else(|| raw.parse::<i64>().ok());
    match parsed {
        Some(n) if n > 0 => n as usize,
        Some(_) => {
            re_log::error!(
                "{key}={raw:?} must be > 0; falling back to default {}",
                re_format::format_bytes(default_bytes as f64),
            );
            default_bytes
        }
        None => {
            re_log::error!(
                "{key}={raw:?} could not be parsed as a byte size (e.g. \"64MB\", \"1GiB\", \
                 or a bare integer number of bytes); falling back to default {}",
                re_format::format_bytes(default_bytes as f64),
            );
            default_bytes
        }
    }
}

/// Parse a fraction in `(0.0, 1.0]` from a pre-trimmed string,
/// falling back to `default` when unparsable, non-finite, or out
/// of range. All failure modes produce an error-level log. Pure
/// function — no env access — so it is safe to unit-test.
fn parse_fraction_or_default(key: &str, raw: &str, default: f64) -> f64 {
    match raw.parse::<f64>() {
        Ok(f) if f.is_finite() && f > 0.0 && f <= 1.0 => f,
        Ok(f) => {
            re_log::error!(
                "{key}={raw:?} must be a finite value in (0.0, 1.0], got {f}; \
                 falling back to default {default}",
            );
            default
        }
        Err(err) => {
            re_log::error!(
                "{key}={raw:?} could not be parsed as a float ({err}); \
                 falling back to default {default}",
            );
            default
        }
    }
}

/// Resolve a byte-size environment variable, falling back to the
/// default (in bytes) when unset, empty, unparsable, or ≤ 0. Accepts
/// either a SI/IEC suffix (`64MB`, `1GiB`) or a bare positive integer
/// interpreted as bytes.
fn read_env_bytes(key: &str, default_bytes: usize) -> usize {
    match read_env_trimmed(key) {
        Some(raw) => parse_bytes_or_default(key, &raw, default_bytes),
        None => default_bytes,
    }
}

/// Resolve a fraction-in-`(0, 1]` environment variable, falling back
/// to the default when unset, empty, unparsable, non-finite, or out
/// of range.
fn read_env_fraction(key: &str, default: f64) -> f64 {
    match read_env_trimmed(key) {
        Some(raw) => parse_fraction_or_default(key, &raw, default),
        None => default,
    }
}

impl PipelineBudget {
    /// Create a new budget derived from `total_uncompressed_estimate`
    /// (clamped per-partition to `[MIN_BUDGET_PER_PARTITION, MAX_BUDGET_PER_PARTITION]`,
    /// then scaled by the number of partitions).
    ///
    /// The clamp bounds and fraction can be overridden at runtime via
    /// [`ENV_BUDGET_MIN`], [`ENV_BUDGET_MAX`], and
    /// [`ENV_BUDGET_FRACTION`]. Invalid values are logged at error
    /// level and the affected parameter falls back to its compile-time
    /// default.
    pub(crate) fn new(total_uncompressed_estimate: usize, num_partitions: usize) -> Self {
        let fraction = read_env_fraction(ENV_BUDGET_FRACTION, BUDGET_FRACTION);
        let mut min_per_partition = read_env_bytes(ENV_BUDGET_MIN, MIN_BUDGET_PER_PARTITION);
        let mut max_per_partition = read_env_bytes(ENV_BUDGET_MAX, MAX_BUDGET_PER_PARTITION);

        if min_per_partition > max_per_partition {
            re_log::error!(
                "{ENV_BUDGET_MIN} ({}) must not exceed {ENV_BUDGET_MAX} ({}); \
                 falling back to defaults for both.",
                re_format::format_bytes(min_per_partition as f64),
                re_format::format_bytes(max_per_partition as f64),
            );
            min_per_partition = MIN_BUDGET_PER_PARTITION;
            max_per_partition = MAX_BUDGET_PER_PARTITION;
        }

        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let per_partition = ((total_uncompressed_estimate as f64 * fraction)
            / num_partitions.max(1) as f64) as usize;
        let budget = per_partition.clamp(min_per_partition, max_per_partition) * num_partitions;

        re_log::debug!("Pipeline budget: {}MB", budget / (1024 * 1024));

        Self::with_exact_budget(budget)
    }

    /// Construct a budget of exactly `budget` bytes with all counters
    /// zeroed. [`Self::new`] derives its `budget` from the adaptive
    /// sizing rules first; tests call this directly to bypass the
    /// env-var lookup and the `MIN_BUDGET_PER_PARTITION` clamp and pin
    /// the budget to a small known value (e.g. for stall-detector
    /// saturation tests where `budget = 100` matters exactly).
    fn with_exact_budget(budget: usize) -> Self {
        Self {
            budget,
            current: AtomicUsize::new(0),
            wait_queue: Mutex::new(BinaryHeap::new()),
            wait_seq: AtomicU64::new(0),
            active_segments: Mutex::new(SegmentGate::default()),
            empty_emit_count: AtomicU32::new(0),
            force_overcommit: AtomicBool::new(false),
            estimate_multiplier: AtomicU64::new(INITIAL_ESTIMATE_MULTIPLIER.to_bits()),
            peak_current: AtomicUsize::new(0),
            total_released_bytes: AtomicUsize::new(0),
            total_releases: AtomicU64::new(0),
            #[cfg(test)]
            test_pause_hook: parking_lot::Mutex::new(None),
        }
    }

    /// Number of [`Self::release`] calls observed since construction.
    /// Test helper for asserting whether a `Drop` / refund path fired,
    /// independent of how many bytes flowed.
    #[cfg(test)]
    pub(crate) fn total_releases(&self) -> u64 {
        self.total_releases.load(Acquire)
    }

    /// Current learned multiplier. Applied to `estimated_bytes` in
    /// `reserve` to derive the reservation size.
    fn current_multiplier(&self) -> f64 {
        f64::from_bits(self.estimate_multiplier.load(Acquire))
    }

    /// Test-only: pin the learned multiplier to a specific value so
    /// tests can exercise the reservation mechanism without the 1.5x
    /// bootstrap multiplier skewing the math.
    ///
    /// `#[cfg(test)]` is sufficient here because all callers live in the
    /// same crate's `tests` module. If other crates ever need to drive
    /// the multiplier from a test, this should grow into a
    /// `#[cfg(any(test, feature = "test_support"))]` shim instead.
    #[cfg(test)]
    fn set_multiplier(&self, multiplier: f64) {
        self.estimate_multiplier
            .store(multiplier.to_bits(), Release);
    }

    /// Test-only: install a pause hook that traps `reserve` between its
    /// rollback `fetch_sub` and `wait_queue.push`. Returns the hook
    /// so the test can await `arrived` (reserver reached the trap) and
    /// later fire `resume` (let it continue).
    #[cfg(test)]
    fn arm_pause_hook(&self) -> TestPauseHook {
        let hook = TestPauseHook {
            arrived: Arc::new(Notify::new()),
            resume: Arc::new(Notify::new()),
        };
        *self.test_pause_hook.lock() = Some(hook.clone());
        hook
    }

    /// Fold a fresh `(estimated, actual)` observation into the EMA.
    /// Raw ratios outside `[MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER]`
    /// are clamped before entering the EMA so a single outlier can't
    /// skew the learned value.
    fn record_actual_sample(&self, estimated: usize, actual: usize) {
        if estimated == 0 {
            return;
        }
        let observed = ((actual as f64) / (estimated as f64))
            .clamp(MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER);
        self.estimate_multiplier
            .fetch_update(AcqRel, Acquire, |bits| {
                let curr = f64::from_bits(bits);
                let next = ESTIMATE_EMA_ALPHA * observed + (1.0 - ESTIMATE_EMA_ALPHA) * curr;
                let next = next.clamp(MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER);
                Some(next.to_bits())
            })
            .expect("closure always returns Some");
    }

    /// Wake the highest-priority parked waiter that can *actually* be
    /// admitted under the current budget + segment-gate state (ties
    /// broken by FIFO).
    ///
    /// Iterating in priority order, this skips two kinds of waiter:
    ///
    /// * [`PriorityWaiter::cancelled`] orphans — reservers that already
    ///   acquired via the post-enqueue `try_admit` race-recovery branch
    ///   and dropped their `notify`. Drained so a cancelled low-time
    ///   orphan doesn't sit at the top of the heap and swallow every
    ///   wake.
    /// * Waiters that are **not currently admittable** — e.g. a
    ///   low-`task_time_min` waiter parked on a full segment-count gate.
    ///   Without this check that waiter would swallow every byte
    ///   `release` wake it can't use, fail its recheck, and re-park
    ///   *without re-delegating* (the wait-loop's fail path does not
    ///   re-wake), stranding the freed bytes until an unrelated wake
    ///   fired — a priority inversion that leans on the stall-breaker to
    ///   recover. Waking the highest-priority *admittable* waiter keeps
    ///   the freed resource flowing while the blocked waiter stays parked
    ///   until its own gate (a [`Self::publish_segment_finalized`]) frees.
    ///
    /// When `force_overcommit` is set every waiter is trivially
    /// admittable, so this degenerates to "wake the strict
    /// highest-priority waiter" — exactly what the stall-breaker wants.
    ///
    /// Admissibility is only a hint: the woken waiter re-validates under
    /// lock in [`Self::try_admit`], so a lost race here costs one extra
    /// park, never correctness.
    fn wake_next(&self) {
        let mut queue = self.wait_queue.lock();
        let segments = self.active_segments.lock();
        let force = self.force_overcommit.load(Acquire);
        let current = self.current.load(Acquire);

        // Pop in priority order; wake the first admittable non-cancelled
        // waiter, hold the rest aside, then restore them. A woken waiter
        // is removed (it re-pushes a fresh entry when it retries), so it
        // is not restored — matching the old pop-and-return contract.
        let mut held: Vec<Reverse<PriorityWaiter>> = Vec::new();
        let mut woke = false;
        while let Some(Reverse(waiter)) = queue.pop() {
            if waiter.cancelled.load(Acquire) {
                continue;
            }
            if !woke && self.waiter_admittable(&waiter, force, current, &segments) {
                waiter.notify.notify_one();
                woke = true;
                continue;
            }
            held.push(Reverse(waiter));
        }
        for h in held {
            queue.push(h);
        }
    }

    /// Whether `waiter` would pass [`Self::try_admit`] given a snapshot
    /// of `force_overcommit`, `current`, and the segment gate. Mirrors
    /// the gate logic in `try_admit` exactly so [`Self::wake_next`]
    /// doesn't waste a wake on a waiter that would immediately re-park.
    fn waiter_admittable(
        &self,
        waiter: &PriorityWaiter,
        force: bool,
        current: usize,
        segments: &SegmentGate,
    ) -> bool {
        if force {
            return true;
        }
        let new_segments = waiter
            .segment_ids
            .iter()
            .filter(|s| !segments.all.contains(s.as_str()))
            .count();
        if segments.effective_len() + new_segments > MAX_CONCURRENT_SEGMENTS {
            return false;
        }
        current + waiter.reserved_bytes <= self.budget
    }

    /// Combined byte + segment-count gate. Returns `Some(new_current)`
    /// when both gates admit `reserved_bytes` AND every segment in
    /// `segment_ids` either already holds a reservation or pushing the
    /// *effective* in-flight count to include them stays within
    /// [`MAX_CONCURRENT_SEGMENTS`].
    ///
    /// Holds the `active_segments` mutex across the byte CAS so the
    /// two gates admit atomically — admitting only the byte gate would
    /// let a fetch stealth-open extra segments past the cap, and
    /// admitting only the segment gate would over-commit bytes.
    ///
    /// Bypassed when `force_overcommit` is set (stall-detection
    /// recovery path). Bypass-admitted segments are tracked in
    /// [`SegmentGate::bypass`] so they don't count against the cap for
    /// future admissions — the cap "self-heals" as soon as those
    /// segments finalize rather than blocking every new admission
    /// until enough segments drain to bring `all.len()` back below
    /// `MAX_CONCURRENT_SEGMENTS`.
    fn try_admit(&self, reserved_bytes: usize, segment_ids: &[String]) -> Option<usize> {
        let mut segments = self.active_segments.lock();

        if self.force_overcommit.load(Acquire) {
            let new_cur = self.current.fetch_add(reserved_bytes, AcqRel) + reserved_bytes;
            self.peak_current.fetch_max(new_cur, AcqRel);
            for s in segment_ids {
                // A segment that's *already* tracked (normal or prior
                // bypass) doesn't add to the bypass overflow. A truly
                // new segment goes into both `all` and `bypass` so
                // future normal admissions can ignore it against the
                // cap.
                if segments.all.insert(s.clone()) {
                    segments.bypass.insert(s.clone());
                }
            }
            return Some(new_cur);
        }

        // How many *new* segments would this admission introduce?
        // Treat the bypass set as if it weren't there — a chunk for a
        // bypass-tracked segment isn't new for cap purposes (the slot
        // is already over-cap and accounted for), but it also doesn't
        // free a normal slot.
        let new_segments = segment_ids
            .iter()
            .filter(|s| !segments.all.contains(s.as_str()))
            .count();
        if segments.effective_len() + new_segments > MAX_CONCURRENT_SEGMENTS {
            return None;
        }

        let new_cur = self.try_acquire(reserved_bytes)?;
        // Bytes admitted; add the new segments. Existing entries
        // (already in the set) re-insert as no-ops.
        segments.all.extend(segment_ids.iter().cloned());
        Some(new_cur)
    }

    /// Try to atomically claim `reserved_bytes` of budget without
    /// inflating `current` past the cap. Returns `Some(new_current)` on
    /// success, `None` if the reservation would push `current` over
    /// `budget`.
    ///
    /// Uses a compare-exchange loop: on contention with other reservers
    /// only the actually-committed reservations move `current`, so
    /// concurrent reservers don't transiently see an over-budget value
    /// and bail out spuriously (the "thundering herd" pattern of the
    /// older `fetch_add → check → fetch_sub` design).
    fn try_acquire(&self, reserved_bytes: usize) -> Option<usize> {
        let previous = self
            .current
            .try_update(AcqRel, Acquire, |cur| {
                let next = cur + reserved_bytes;
                (next <= self.budget).then_some(next)
            })
            .ok()?;
        let current = previous + reserved_bytes;

        self.peak_current.fetch_max(current, AcqRel);
        Some(current)
    }

    /// Single-arg wrapper kept for backward compatibility with the
    /// internal tests that pre-date the priority/gate work. New code
    /// must use [`Self::reserve_with_priority`] so the parked waiter
    /// gets a meaningful priority and the segment-count gate is fed.
    #[cfg(test)]
    pub(crate) async fn reserve(&self, estimated_bytes: usize) -> usize {
        self.reserve_with_priority(estimated_bytes, TimeInt::MAX, &[])
            .await
    }

    /// Atomically reserve budget space before fetching, sized from
    /// `estimated_bytes` scaled by the learned estimate→actual multiplier.
    /// Blocks if the byte budget *or* the segment-count gate would be
    /// exceeded.
    ///
    /// `task_time_min` is the smallest `time_min` across the chunks
    /// this reservation is acquiring bytes for, on the query's
    /// `filtered_index` timeline. It controls the parked-waiter
    /// priority: earlier-time fetches wake first, so when the CPU
    /// worker's safe horizon is gated on a slow-arriving early chunk
    /// that fetch preempts later-time fetches contending for the same
    /// budget. Pass [`TimeInt::MAX`] for fetches with no temporal
    /// info (static-only, old server) to put them at the back of the
    /// heap.
    ///
    /// `segment_ids` is the set of distinct `segment_id`s this fetch
    /// produces chunks for. Admitted atomically with the byte
    /// reservation so the segment-count gate cannot be bypassed by
    /// the multi-segment batches `create_request_batches` emits.
    ///
    /// Returns the actual reserved byte count so the caller can pass
    /// it back into [`adjust_reservation`](Self::adjust_reservation)
    /// alongside the measured decoded size.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) async fn reserve_with_priority(
        &self,
        estimated_bytes: usize,
        task_time_min: TimeInt,
        segment_ids: &[String],
    ) -> usize {
        let reserved_bytes = ((estimated_bytes as f64) * self.current_multiplier()) as usize;

        if reserved_bytes > self.budget {
            re_log::warn!(
                "Single fetch reservation ({}MB, raw estimate {}MB) exceeds entire \
                 pipeline budget ({}MB across all partitions) — allowing it through \
                 to avoid deadlock.",
                reserved_bytes / (1024 * 1024),
                estimated_bytes / (1024 * 1024),
                self.budget / (1024 * 1024),
            );
            let new_cur = self.current.fetch_add(reserved_bytes, AcqRel) + reserved_bytes;
            self.peak_current.fetch_max(new_cur, AcqRel);
            // This is a deadlock-avoidance escape hatch — the fetch's
            // segments are intentionally admitted past the cap, same
            // as the `force_overcommit` bypass. Track them as bypass
            // so the cap self-heals on finalization.
            let mut segments = self.active_segments.lock();
            for s in segment_ids {
                if segments.all.insert(s.clone()) {
                    segments.bypass.insert(s.clone());
                }
            }
            return reserved_bytes;
        }

        re_log::debug_assert!(
            segment_ids
                .iter()
                .enumerate()
                .all(|(idx, segment_id)| !segment_ids[..idx].contains(segment_id)),
            "segment_ids must be distinct"
        );
        let distinct_segments = segment_ids.len();
        if distinct_segments > MAX_CONCURRENT_SEGMENTS {
            re_log::warn_once!(
                "Single fetch reservation spans {distinct_segments} distinct segments, \
                 exceeding the concurrent-segment cap ({MAX_CONCURRENT_SEGMENTS}) — allowing \
                 it through to avoid deadlock.",
            );
            let new_cur = self.current.fetch_add(reserved_bytes, AcqRel) + reserved_bytes;
            self.peak_current.fetch_max(new_cur, AcqRel);
            let mut segments = self.active_segments.lock();
            for s in segment_ids {
                if segments.all.insert(s.clone()) {
                    segments.bypass.insert(s.clone());
                }
            }
            return reserved_bytes;
        }

        let mut wait_count: u32 = 0;
        loop {
            // Fast path: combined byte + segment-count gate.
            if let Some(new_cur) = self.try_admit(reserved_bytes, segment_ids) {
                if new_cur < self.budget {
                    self.wake_next();
                }
                if wait_count > 0 {
                    re_log::debug!(
                        "Budget reserve succeeded after {wait_count} waits: \
                         reserved {}MB, current {}MB / {}MB",
                        reserved_bytes / (1024 * 1024),
                        new_cur / (1024 * 1024),
                        self.budget / (1024 * 1024),
                    );
                }
                return reserved_bytes;
            }

            // Slow path: park. Enqueue the priority waiter *before*
            // awaiting and re-try `try_admit` after enqueuing. This
            // closes the lost-wakeup race: if `release` /
            // `adjust_reservation` /
            // `publish_segment_finalized` runs between our initial
            // failure and the `push`, the second `try_admit`
            // observes the freed slot and we proceed without ever
            // awaiting; otherwise the release pops our notify and
            // stores the permit, so the subsequent
            // `notified().await` returns immediately.
            let notify = Arc::new(Notify::new());
            let cancelled = Arc::new(AtomicBool::new(false));
            let seq = self.wait_seq.fetch_add(1, AcqRel);
            self.wait_queue.lock().push(Reverse(PriorityWaiter {
                task_time_min,
                seq,
                notify: Arc::clone(&notify),
                cancelled: Arc::clone(&cancelled),
                reserved_bytes,
                segment_ids: segment_ids.to_vec(),
            }));

            #[cfg(test)]
            {
                let hook = self.test_pause_hook.lock().clone();
                if let Some(hook) = hook {
                    hook.arrived.notify_one();
                    hook.resume.notified().await;
                }
            }

            if let Some(new_cur) = self.try_admit(reserved_bytes, segment_ids) {
                // Acquired between our wait decision and enqueue. Mark
                // the just-pushed waiter cancelled so `wake_next`
                // drains it instead of wasting a wake on our dropped
                // Arc — otherwise a low-`task_time_min` orphan sitting
                // at the top of the priority heap would steal wake
                // events from genuine parked waiters with higher
                // `task_time_min`.
                cancelled.store(true, Release);
                if new_cur < self.budget {
                    self.wake_next();
                }
                return reserved_bytes;
            }

            wait_count += 1;
            if wait_count == 1 || wait_count.is_multiple_of(10) {
                // info-level so it's visible for tuning MAX_BUDGET_PER_PARTITION
                // without needing a RUST_LOG=debug setup.
                let segments = self.active_segments.lock();
                re_log::info!(
                    "Budget backpressure (wait #{wait_count}): want {}MB, \
                     current {}MB / {}MB budget, active_segments={} (bypass={})",
                    reserved_bytes / (1024 * 1024),
                    self.current.load(Acquire) / (1024 * 1024),
                    self.budget / (1024 * 1024),
                    segments.all.len(),
                    segments.bypass.len(),
                );
            }

            notify.notified().await;
        }
    }

    /// Adjust a prior reservation to reflect the actual decoded size.
    /// Call after fetch completes. `reserved` is the value returned by
    /// [`reserve_with_priority`](Self::reserve_with_priority); `estimated` is the raw uncompressed size
    /// that was passed in (used to train the multiplier). If `actual >
    /// reserved` this adds the delta to current; if `actual < reserved`
    /// this subtracts (saturating to avoid underflow from concurrent
    /// [`release`](Self::release) calls) and wakes a waiter.
    ///
    /// A shrink also resets the stall detector — `current` dropping
    /// without a corresponding [`Self::release`] would otherwise leave
    /// `force_overcommit` stale-armed across a now-unsaturated
    /// window, and the next `reserve` would bypass both gates with no
    /// real stall to break. Reset matches what [`Self::release`] does
    /// on every byte-freeing path.
    ///
    /// Also folds the `(estimated, actual)` observation into the learned
    /// estimate→actual multiplier via EMA so subsequent reservations
    /// size closer to the true decoded footprint.
    pub(crate) fn adjust_reservation(&self, estimated: usize, reserved: usize, actual: usize) {
        if actual > reserved {
            let new_cur = self.current.fetch_add(actual - reserved, AcqRel) + (actual - reserved);
            self.peak_current.fetch_max(new_cur, AcqRel);
        } else if reserved > actual {
            self.current
                .fetch_update(AcqRel, Acquire, |current| {
                    Some(current.saturating_sub(reserved - actual))
                })
                .expect("closure always returns Some");
            // Budget bytes were just freed — same progress signal as
            // `release`. Clear the stall detector so a now-stale
            // `force_overcommit` can't make the next `reserve`
            // wrongly bypass both gates.
            self.empty_emit_count.store(0, Release);
            self.force_overcommit.store(false, Release);
            self.wake_next();
        }
        self.record_actual_sample(estimated, actual);
    }

    /// Release decoded bytes from the pipeline.
    ///
    /// Called by the CPU thread once a segment's `CurrentStores` has been
    /// flushed and dropped — its chunks are no longer in memory at this
    /// point, so we can safely return their reserved bytes to the budget.
    ///
    /// Uses `fetch_update` with `saturating_sub` to avoid underflow when concurrent
    /// operations have already reduced `current` below `bytes`.
    pub(crate) fn release(&self, bytes: usize) {
        let prev = self
            .current
            .fetch_update(AcqRel, Acquire, |current| {
                Some(current.saturating_sub(bytes))
            })
            .expect("closure always returns Some");
        self.total_released_bytes.fetch_add(bytes, AcqRel);
        self.total_releases.fetch_add(1, AcqRel);
        // Real progress: budget bytes were just freed, so the stall
        // detector should reset.
        self.empty_emit_count.store(0, Release);
        self.force_overcommit.store(false, Release);
        // Per-call detail at debug level only — high-throughput queries
        // emit one of these per segment per partition. The aggregate
        // peak / cumulative-released numbers used for tuning live in
        // the `Drop` summary below, so the per-call line is no longer
        // info-worthy in production.
        re_log::debug!(
            "Budget release: freed {}MB, {}MB → {}MB / {}MB",
            bytes / (1024 * 1024),
            prev / (1024 * 1024),
            prev.saturating_sub(bytes) / (1024 * 1024),
            self.budget / (1024 * 1024),
        );
        // Wake the highest-priority waiter so it can retry.
        self.wake_next();
    }

    /// Remove `segment_id` from the segment-count gate and wake the
    /// highest-priority parked waiter. Called from
    /// `Drop for CurrentStores` once the CPU worker is fully done
    /// with a segment.
    ///
    /// Decoupled from [`Self::release`] because byte release happens
    /// incrementally (via `gc_up_to_horizon` as the safe horizon
    /// advances) while the segment's "slot" in the segment-count
    /// gate is held until the CPU worker fully drops the segment.
    pub(crate) fn publish_segment_finalized(&self, segment_id: &str) {
        let mut segments = self.active_segments.lock();
        // Remove from `bypass` too so the effective_len reflects the
        // departure correctly. The two cases:
        //   * Normal segment: `bypass.remove` is a no-op, `all.len()`
        //     drops by 1, effective_len drops by 1 → a real slot frees
        //     and a parked normal waiter can advance.
        //   * Bypass segment: `all.len()` and `bypass.len()` both drop
        //     by 1, effective_len unchanged → no extra slot opens,
        //     but the bypass overflow shrinks toward zero so the cap
        //     contract heals.
        let was_bypass = segments.bypass.remove(segment_id);
        let removed = segments.all.remove(segment_id);
        drop(segments);
        if removed || was_bypass {
            self.wake_next();
        }
    }

    /// Count a `flush_incremental` call that produced no emittable
    /// rows. When the count crosses [`STALL_EMPTY_EMIT_THRESHOLD`]
    /// *consecutive* saturated empty emits, sets `force_overcommit`
    /// so the next `reserve` bypasses both gates. The bypass lets a
    /// parked horizon-advancing fetch land on the CPU side and free
    /// real work; once that fetch's decoded bytes land and a real
    /// `release` follows, the detector resets.
    ///
    /// Saturation is checked *first* and the counter is reset to zero
    /// whenever the budget is below [`STALL_SATURATION_THRESHOLD`]:
    /// without that gate, empty emits during slow-manifest startup
    /// (budget empty, nothing to flush) would pump the counter past
    /// threshold so the very first saturated cycle would trip the
    /// breaker — a false positive, not a real stall.
    pub(crate) fn notify_empty_emit(&self) {
        #[expect(clippy::cast_precision_loss)]
        let saturation = (self.current.load(Acquire) as f64) / (self.budget.max(1) as f64);
        if saturation < STALL_SATURATION_THRESHOLD {
            // Unsaturated: not a stall candidate. Reset so the counter
            // strictly measures *consecutive saturated* empty emits.
            self.empty_emit_count.store(0, Release);
            return;
        }
        let count = self.empty_emit_count.fetch_add(1, AcqRel) + 1;
        // CAS so exactly one caller arms the breaker (and logs / wakes)
        // when several CPU workers sharing the budget cross the
        // threshold together.
        if count >= STALL_EMPTY_EMIT_THRESHOLD
            && self
                .force_overcommit
                .compare_exchange(false, true, AcqRel, Relaxed)
                .is_ok()
        {
            re_log::info!(
                "PipelineBudget stall detected: {count} consecutive empty emits with \
                 budget {:.0}% saturated — enabling force_overcommit until next progress",
                saturation * 100.0,
            );
            // Wake whoever's at the front so they can break out
            // through the bypass.
            self.wake_next();
        }
    }

    /// Reset the stall detector on real CPU-side progress. Called by
    /// the worker each time a `RecordBatch` is actually emitted.
    pub(crate) fn notify_row_emitted(&self) {
        self.empty_emit_count.store(0, Release);
        self.force_overcommit.store(false, Release);
    }

    /// Return a reservation to the budget without recording an EMA sample.
    ///
    /// Used by [`ReservationGuard::drop`] on error / early-return paths
    /// where the fetch never produced a decoded byte count, so we have
    /// nothing meaningful to teach the EMA. Saturates on underflow to
    /// match [`Self::release`].
    fn refund_reservation(&self, reserved: usize) {
        if reserved == 0 {
            return;
        }
        self.current
            .fetch_update(AcqRel, Acquire, |current| {
                Some(current.saturating_sub(reserved))
            })
            .expect("closure always returns Some");
        self.wake_next();
    }

    /// Reserve like [`reserve`](Self::reserve) and wrap the result in a
    /// [`ReservationGuard`].
    ///
    /// The guard returns the full reservation to the budget on drop unless
    /// the caller calls [`ReservationGuard::commit`] with the actual
    /// decoded byte count. This is the preferred API for call sites that
    /// have fallible work (`?`, `.await?`) between `reserve` and the
    /// decoded-size measurement: an early return on those paths would
    /// otherwise leak the reservation and permanently reduce headroom for
    /// other partitions sharing the same budget.
    #[cfg(test)]
    pub(crate) async fn reserve_guarded(&self, estimated: usize) -> ReservationGuard<'_> {
        self.reserve_guarded_with_priority(estimated, TimeInt::MAX, Vec::new())
            .await
    }

    pub(crate) async fn reserve_guarded_with_priority(
        &self,
        estimated: usize,
        task_time_min: TimeInt,
        segment_ids: Vec<String>,
    ) -> ReservationGuard<'_> {
        let reserved = self
            .reserve_with_priority(estimated, task_time_min, &segment_ids)
            .await;
        ReservationGuard {
            budget: self,
            estimated,
            reserved,
            segment_ids,
            committed: false,
        }
    }
}

/// RAII guard for a [`PipelineBudget`] reservation.
///
/// Returned by [`PipelineBudget::reserve_guarded_with_priority`]. Call [`Self::commit`]
/// with the actual decoded byte count once known to fold the observation
/// into the budget's EMA. Dropping without committing returns the entire
/// reservation — both the reserved bytes AND the segment-count gate slots
/// this fetch admitted — as if the fetch produced zero chunks. Used to
/// recover headroom on error / early-return paths.
///
/// The segment slots matter as much as the bytes: a failed fetch's
/// segments never reach the CPU worker, so the `Drop for CurrentStores`
/// path that normally calls [`PipelineBudget::publish_segment_finalized`]
/// never runs for them. Without vacating here they leak permanently, and
/// since the segment-count gate stays engaged even when the byte budget
/// is sized wide open, a handful of failed fetches on distinct segments
/// can wedge the gate shut for every later reservation.
#[must_use = "ReservationGuard returns its bytes and segment slots to the budget on drop; \
              call .commit(actual) once the decoded size is known"]
pub(crate) struct ReservationGuard<'a> {
    budget: &'a PipelineBudget,
    estimated: usize,
    reserved: usize,

    /// Distinct segments this reservation admitted into the gate. Vacated
    /// on uncommitted drop; on commit the segments stay active until the
    /// CPU worker finalizes them via `publish_segment_finalized`.
    segment_ids: Vec<String>,
    committed: bool,
}

impl ReservationGuard<'_> {
    /// Commit the reservation against the actual decoded byte count.
    /// Folds the `(estimated, actual)` observation into the budget's
    /// EMA and consumes the guard so its `Drop` becomes a no-op.
    pub(crate) fn commit(mut self, actual: usize) {
        self.budget
            .adjust_reservation(self.estimated, self.reserved, actual);
        self.committed = true;
    }
}

impl Drop for ReservationGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            // Caller dropped without commit — error / panic / early
            // return. Return the full reservation to the budget without
            // folding a 0-byte sample into the EMA: a failed fetch
            // observed nothing about decode ratios and shouldn't drag
            // the learned multiplier toward zero.
            self.budget.refund_reservation(self.reserved);
            // Vacate the segment-count gate slots too. These segments
            // never reached the CPU worker, so the `CurrentStores` drop
            // that normally finalizes them won't run — without this they
            // leak from the gate forever. `publish_segment_finalized` is
            // a no-op for any segment a concurrent owner already removed.
            for segment_id in &self.segment_ids {
                self.budget.publish_segment_finalized(segment_id);
            }
        }
    }
}

impl Drop for PipelineBudget {
    /// One-shot lifecycle summary at info level so peak / total numbers
    /// are visible after a query without needing `RUST_LOG=debug`.
    /// Skipped when the budget was never used (e.g. construction-only
    /// in tests) to keep test output quiet.
    fn drop(&mut self) {
        let n_releases = self.total_releases.load(Acquire);
        if n_releases == 0 {
            return;
        }
        const MB: usize = 1024 * 1024;
        let peak = self.peak_current.load(Acquire);
        let total_released = self.total_released_bytes.load(Acquire);
        let pct = if self.budget > 0 {
            #[expect(clippy::cast_precision_loss)]
            let pct = peak as f64 / self.budget as f64 * 100.0;
            pct
        } else {
            0.0
        };
        re_log::info!(
            "PipelineBudget summary: peak={}MB / {}MB ({pct:.0}%), \
             released_total={}MB across {n_releases} calls",
            peak / MB,
            self.budget / MB,
            total_released / MB,
        );
    }
}

impl std::fmt::Debug for PipelineBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineBudget")
            .field("budget", &self.budget)
            .field("current", &self.current.load(Relaxed))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
