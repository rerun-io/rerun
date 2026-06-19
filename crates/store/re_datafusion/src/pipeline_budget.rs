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
//! 1. **Before fetch:** IO task calls [`PipelineBudget::reserve`] with the
//!    chunk's uncompressed size. If the budget is full, the call blocks until
//!    budget space frees up. A parked reserver wakes when any of:
//!    - [`PipelineBudget::release`] runs on the CPU thread,
//!    - [`PipelineBudget::adjust_reservation`] shrinks an earlier reservation,
//!    - another `reserve` succeeds with remaining headroom and cascade-wakes
//!      the next waiter.
//! 2. **After fetch:** IO task calls [`PipelineBudget::adjust_reservation`] to
//!    correct the estimate to the actual decoded Arrow heap size.
//! 3. **After segment finalization:** CPU thread calls
//!    [`PipelineBudget::release`] to return freed bytes to the budget, waking
//!    any blocked IO tasks.
//!
//! # Exhaustion behavior
//!
//! When `current >= budget` and a new [`PipelineBudget::reserve`] arrives, the
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
//!    pushes it onto a FIFO `wait_queue`, then **rechecks** `try_acquire` once
//!    more before awaiting. The recheck closes a lost-wakeup race: if a
//!    [`PipelineBudget::release`] / [`PipelineBudget::adjust_reservation`]
//!    fires between the initial fast-path miss and the enqueue, the recheck
//!    observes the freed budget and the task proceeds without ever awaiting.
//!    The first park and every tenth re-park emit an info-level backpressure
//!    log so the condition is visible in production without needing
//!    `RUST_LOG=debug`.
//!
//! Parked reservers wake from any of three sources:
//!
//! - [`PipelineBudget::release`] (CPU thread, at segment finalization) —
//!   the dominant source under steady-state load.
//! - [`PipelineBudget::adjust_reservation`] when `actual < reserved` (IO
//!   thread, on fetch completion that under-ran its reservation) — refunds
//!   the sliver between the multiplier-scaled estimate and the measured
//!   decoded size.
//! - Cascade-wake from a sibling `reserve` that succeeded with remaining
//!   headroom, ensuring a chain of small reservations doesn't strand a
//!   single large waiter at the head of the queue.
//!
//! Wake-up order is strict FIFO via `VecDeque::pop_front`. A woken task
//! re-enters the acquire loop from the top: a successful claim returns; a
//! still-insufficient claim re-enqueues a fresh `Notify` and re-parks.
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

use std::collections::VecDeque;
use std::sync::Arc;

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

/// Tracks total decoded bytes in the pipeline and enforces a memory budget.
///
/// See the [module-level documentation](self) for the full design.
pub(crate) struct PipelineBudget {
    /// Maximum decoded bytes allowed in the pipeline at any time.
    budget: usize,

    /// Current decoded bytes in the pipeline (IO buffers + channel + `ChunkStore`).
    current: std::sync::atomic::AtomicUsize,

    /// FIFO queue of parked reserve-waiters. `release` and
    /// `adjust_reservation` wake the oldest waiter first.
    wait_queue: Mutex<VecDeque<Arc<Notify>>>,

    /// Learned multiplier applied to `reserve` estimates so reservations
    /// track true decoded size rather than the raw (often low)
    /// uncompressed estimate. Stored as `f64::to_bits` in an atomic so
    /// `reserve` can read lock-free; updated via a CAS loop in
    /// `adjust_reservation`. Starts at [`INITIAL_ESTIMATE_MULTIPLIER`]
    /// and converges via EMA toward the dataset's actual
    /// `actual / estimated` ratio, clamped to
    /// `[MIN_ESTIMATE_MULTIPLIER, MAX_ESTIMATE_MULTIPLIER]`.
    estimate_multiplier: std::sync::atomic::AtomicU64,

    /// Highest value `current` ever reached during the lifetime of
    /// this budget. Used in the lifecycle summary emitted on `Drop`.
    peak_current: std::sync::atomic::AtomicUsize,

    /// Cumulative bytes ever passed to [`Self::release`]. Lifecycle
    /// summary diagnostic only.
    total_released_bytes: std::sync::atomic::AtomicUsize,

    /// Number of [`Self::release`] calls. Lifecycle summary
    /// diagnostic only.
    total_releases: std::sync::atomic::AtomicU64,

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

        Self {
            budget,
            current: std::sync::atomic::AtomicUsize::new(0),
            wait_queue: Mutex::new(VecDeque::new()),
            estimate_multiplier: std::sync::atomic::AtomicU64::new(
                INITIAL_ESTIMATE_MULTIPLIER.to_bits(),
            ),
            peak_current: std::sync::atomic::AtomicUsize::new(0),
            total_released_bytes: std::sync::atomic::AtomicUsize::new(0),
            total_releases: std::sync::atomic::AtomicU64::new(0),
            #[cfg(test)]
            test_pause_hook: parking_lot::Mutex::new(None),
        }
    }

    /// Number of [`Self::release`] calls observed since construction.
    /// Test helper for asserting whether a `Drop` / refund path fired,
    /// independent of how many bytes flowed.
    #[cfg(test)]
    pub(crate) fn total_releases(&self) -> u64 {
        self.total_releases
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// Current learned multiplier. Applied to `estimated_bytes` in
    /// `reserve` to derive the reservation size.
    fn current_multiplier(&self) -> f64 {
        f64::from_bits(
            self.estimate_multiplier
                .load(std::sync::atomic::Ordering::Acquire),
        )
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
            .store(multiplier.to_bits(), std::sync::atomic::Ordering::Release);
    }

    /// Test-only: install a pause hook that traps `reserve` between its
    /// rollback `fetch_sub` and `wait_queue.push_back`. Returns the hook
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
        use std::sync::atomic::Ordering::{AcqRel, Acquire};
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

    /// Wake the oldest parked waiter.
    fn wake_next(&self) {
        if let Some(notify) = self.wait_queue.lock().pop_front() {
            notify.notify_one();
        }
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
        use std::sync::atomic::Ordering::{AcqRel, Acquire};
        let mut cur = self.current.load(Acquire);
        loop {
            let next = cur + reserved_bytes;
            if next > self.budget {
                return None;
            }
            match self
                .current
                .compare_exchange_weak(cur, next, AcqRel, Acquire)
            {
                Ok(_) => {
                    self.peak_current.fetch_max(next, AcqRel);
                    return Some(next);
                }
                Err(actual) => cur = actual,
            }
        }
    }

    /// Atomically reserve budget space before fetching, sized from
    /// `estimated_bytes` scaled by the learned estimate→actual multiplier.
    /// Blocks if the budget would be exceeded. Returns the actual
    /// reserved byte count so the caller can pass it back into
    /// [`adjust_reservation`](Self::adjust_reservation) alongside the
    /// measured decoded size.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) async fn reserve(&self, estimated_bytes: usize) -> usize {
        use std::sync::atomic::Ordering::AcqRel;

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
            return reserved_bytes;
        }

        let mut wait_count: u32 = 0;
        loop {
            // Fast path: try to reserve without ever inflating `current`.
            if let Some(new_cur) = self.try_acquire(reserved_bytes) {
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

            // Slow path: park. Enqueue the notify *before* awaiting and
            // re-try the acquire after enqueuing. This closes the
            // lost-wakeup race: if `release` / `adjust_reservation` runs
            // between our initial `try_acquire` failure and the
            // `push_back`, the second `try_acquire` observes the freed
            // budget and we proceed without ever awaiting; if the
            // release runs after `push_back` instead, it pops our
            // notify and stores the permit, so the subsequent
            // `notified().await` returns immediately.
            let notify = Arc::new(Notify::new());
            self.wait_queue.lock().push_back(Arc::clone(&notify));

            #[cfg(test)]
            {
                let hook = self.test_pause_hook.lock().clone();
                if let Some(hook) = hook {
                    hook.arrived.notify_one();
                    hook.resume.notified().await;
                }
            }

            if let Some(new_cur) = self.try_acquire(reserved_bytes) {
                // Acquired between our wait decision and enqueue. Any
                // future `wake_next` may pop our orphan notify and fire
                // `notify_one` into the dropped Arc — harmless.
                if new_cur < self.budget {
                    self.wake_next();
                }
                return reserved_bytes;
            }

            wait_count += 1;
            if wait_count == 1 || wait_count.is_multiple_of(10) {
                // info-level so it's visible for tuning MAX_BUDGET_PER_PARTITION
                // without needing a RUST_LOG=debug setup.
                re_log::info!(
                    "Budget backpressure (wait #{wait_count}): want {}MB, \
                     current {}MB / {}MB budget",
                    reserved_bytes / (1024 * 1024),
                    self.current.load(std::sync::atomic::Ordering::Acquire) / (1024 * 1024),
                    self.budget / (1024 * 1024),
                );
            }

            notify.notified().await;
        }
    }

    /// Adjust a prior reservation to reflect the actual decoded size.
    /// Call after fetch completes. `reserved` is the value returned by
    /// [`reserve`](Self::reserve); `estimated` is the raw uncompressed size
    /// that was passed in (used to train the multiplier). If `actual >
    /// reserved` this adds the delta to current; if `actual < reserved`
    /// this subtracts (saturating to avoid underflow from concurrent
    /// [`release`](Self::release) calls) and wakes a waiter.
    ///
    /// Also folds the `(estimated, actual)` observation into the learned
    /// estimate→actual multiplier via EMA so subsequent reservations
    /// size closer to the true decoded footprint.
    pub(crate) fn adjust_reservation(&self, estimated: usize, reserved: usize, actual: usize) {
        use std::sync::atomic::Ordering::{AcqRel, Acquire};
        if actual > reserved {
            let new_cur = self.current.fetch_add(actual - reserved, AcqRel) + (actual - reserved);
            self.peak_current.fetch_max(new_cur, AcqRel);
        } else if reserved > actual {
            self.current
                .fetch_update(AcqRel, Acquire, |current| {
                    Some(current.saturating_sub(reserved - actual))
                })
                .expect("closure always returns Some");
            // Freed budget space — wake a waiter.
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
        use std::sync::atomic::Ordering::{AcqRel, Acquire};
        let prev = self
            .current
            .fetch_update(AcqRel, Acquire, |current| {
                Some(current.saturating_sub(bytes))
            })
            .expect("closure always returns Some");
        self.total_released_bytes.fetch_add(bytes, AcqRel);
        self.total_releases.fetch_add(1, AcqRel);
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
        // Wake the oldest waiter so it can retry.
        self.wake_next();
    }

    /// Return a reservation to the budget without recording an EMA sample.
    ///
    /// Used by [`ReservationGuard::drop`] on error / early-return paths
    /// where the fetch never produced a decoded byte count, so we have
    /// nothing meaningful to teach the EMA. Saturates on underflow to
    /// match [`Self::release`].
    fn refund_reservation(&self, reserved: usize) {
        use std::sync::atomic::Ordering::{AcqRel, Acquire};
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
    pub(crate) async fn reserve_guarded(&self, estimated: usize) -> ReservationGuard<'_> {
        let reserved = self.reserve(estimated).await;
        ReservationGuard {
            budget: self,
            estimated,
            reserved,
            committed: false,
        }
    }
}

/// RAII guard for a [`PipelineBudget`] reservation.
///
/// Returned by [`PipelineBudget::reserve_guarded`]. Call [`Self::commit`]
/// with the actual decoded byte count once known to fold the observation
/// into the budget's EMA. Dropping without committing returns the entire
/// reservation as if the fetch produced zero bytes — used to recover
/// headroom on error / early-return paths.
#[must_use = "ReservationGuard returns its bytes to the budget on drop; \
              call .commit(actual) once the decoded size is known"]
pub(crate) struct ReservationGuard<'a> {
    budget: &'a PipelineBudget,
    estimated: usize,
    reserved: usize,
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
        }
    }
}

impl Drop for PipelineBudget {
    /// One-shot lifecycle summary at info level so peak / total numbers
    /// are visible after a query without needing `RUST_LOG=debug`.
    /// Skipped when the budget was never used (e.g. construction-only
    /// in tests) to keep test output quiet.
    fn drop(&mut self) {
        use std::sync::atomic::Ordering::Acquire;
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
            .field(
                "current",
                &self.current.load(std::sync::atomic::Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    // Default constants are tuned to leave the budget effectively
    // disengaged (FRACTION=1.0, MIN=4 GiB, MAX=1 TiB). The tests below
    // assert the clamp logic still selects the right bound at the
    // extremes — not that the budget meaningfully restricts in-flight
    // bytes at typical data sizes. Once the CPU-worker streaming-release
    // refactor lands and the constants come back down (FRACTION=0.25,
    // MIN=64 MiB, MAX=1 GiB), tighten these to also assert proportional
    // sizing in the small/medium ranges.

    #[test]
    fn test_budget_clamps_to_min() {
        // 10 MB total, 1 partition → 100% = 10 MB, clamped to MIN_BUDGET_PER_PARTITION (4 GiB)
        let budget = PipelineBudget::new(10 * 1024 * 1024, 1);
        assert_eq!(budget.budget, MIN_BUDGET_PER_PARTITION);
    }

    #[test]
    fn test_budget_clamps_to_max() {
        // 8 PiB total, 1 partition → 100% = 8 PiB, clamped to MAX_BUDGET_PER_PARTITION (1 TiB)
        let budget = PipelineBudget::new(8 * 1024 * 1024 * 1024 * 1024 * 1024, 1);
        assert_eq!(budget.budget, MAX_BUDGET_PER_PARTITION);
    }

    #[test]
    fn test_budget_scales_with_partitions() {
        // 64 PiB total, 14 partitions → 100% / 14 ≈ 4.6 PiB per partition,
        // clamped to MAX_BUDGET_PER_PARTITION (1 TiB) each.
        let budget = PipelineBudget::new(64 * 1024 * 1024 * 1024 * 1024 * 1024, 14);
        assert_eq!(budget.budget, MAX_BUDGET_PER_PARTITION * 14);
    }

    #[test]
    fn test_budget_small_data_many_partitions() {
        // 100 MB total, 4 partitions → 100% / 4 = 25 MB per partition,
        // clamped to MIN_BUDGET_PER_PARTITION (4 GiB) each.
        let budget = PipelineBudget::new(100 * 1024 * 1024, 4);
        assert_eq!(budget.budget, MIN_BUDGET_PER_PARTITION * 4);
    }

    #[tokio::test]
    async fn test_reserve_blocks_when_budget_exhausted() {
        let budget = Arc::new(PipelineBudget::new(0, 1)); // MIN_BUDGET = 64 MB
        budget.set_multiplier(1.0);
        let half = budget.budget / 2;

        // First reserve should succeed immediately
        budget.reserve(half).await;
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            half
        );

        // Second reserve should also succeed (half + half = budget)
        budget.reserve(half).await;
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            half * 2
        );

        // Third reserve should block because budget is full.
        let budget_clone = Arc::clone(&budget);
        let handle = tokio::spawn(async move {
            budget_clone.reserve(half).await;
        });

        // Give the task a chance to run (it should be blocked)
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !handle.is_finished(),
            "reserve should block when budget is exhausted"
        );

        // Release enough to unblock
        budget.release(half);

        // Now the spawned task should complete
        tokio::time::timeout(std::time::Duration::from_secs(1), handle)
            .await
            .expect("reserve should unblock after release")
            .expect("task should not panic");
    }

    #[tokio::test]
    async fn test_adjust_reservation_corrects_estimate() {
        let budget = Arc::new(PipelineBudget::new(0, 1)); // MIN_BUDGET = 64 MB
        budget.set_multiplier(1.0);

        let estimated = 1000;
        let actual = 600;

        let reserved = budget.reserve(estimated).await;
        assert_eq!(reserved, estimated);
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            estimated
        );

        budget.adjust_reservation(estimated, reserved, actual);
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            actual
        );
    }

    #[tokio::test]
    async fn test_estimate_multiplier_adapts_over_time() {
        // With the 1.5x bootstrap, a task estimating 1000 bytes initially
        // reserves 1500. As we feed back samples showing actual is 2x the
        // raw estimate, the learned multiplier climbs toward 2.0 and
        // subsequent reserves size accordingly.
        let budget = Arc::new(PipelineBudget::new(0, 1));
        assert_eq!(budget.current_multiplier(), INITIAL_ESTIMATE_MULTIPLIER);

        let estimated = 1000;
        let actual = 2000; // true ratio = 2.0

        // First sample: reserve under bootstrap, then teach the EMA.
        let reserved1 = budget.reserve(estimated).await;
        assert_eq!(reserved1, 1500);
        budget.adjust_reservation(estimated, reserved1, actual);
        budget.release(actual);

        // Multiplier has nudged toward 2.0 but isn't there yet
        // (EMA: 0.2 * 2.0 + 0.8 * 1.5 = 1.6).
        assert!((budget.current_multiplier() - 1.6).abs() < 1e-9);

        // After many samples at ratio=2.0 the multiplier converges.
        for _ in 0..40 {
            let reserved = budget.reserve(estimated).await;
            budget.adjust_reservation(estimated, reserved, actual);
            budget.release(actual);
        }
        assert!(
            (budget.current_multiplier() - 2.0).abs() < 0.01,
            "multiplier should converge toward 2.0, got {}",
            budget.current_multiplier()
        );
    }

    #[tokio::test]
    async fn test_estimate_multiplier_is_clamped() {
        // A pathological 10x expansion should not blow out the multiplier;
        // it saturates at MAX_ESTIMATE_MULTIPLIER. Raw samples are clamped
        // before entering the EMA, so the EMA itself converges toward the
        // clamped value (3.0) rather than the raw ratio (10.0).
        let budget = Arc::new(PipelineBudget::new(0, 1));
        for _ in 0..100 {
            budget.record_actual_sample(100, 1000); // raw ratio = 10
        }
        assert!(
            (budget.current_multiplier() - MAX_ESTIMATE_MULTIPLIER).abs() < 1e-4,
            "multiplier should converge to MAX, got {}",
            budget.current_multiplier()
        );

        // And undersized actual (ratio < 1) floors at MIN_ESTIMATE_MULTIPLIER.
        for _ in 0..100 {
            budget.record_actual_sample(1000, 100); // raw ratio = 0.1
        }
        assert!(
            (budget.current_multiplier() - MIN_ESTIMATE_MULTIPLIER).abs() < 1e-4,
            "multiplier should converge to MIN, got {}",
            budget.current_multiplier()
        );
    }

    #[tokio::test]
    async fn test_peak_current_tracks_high_water_mark() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);

        // Reserve, release, reserve smaller — peak should reflect the
        // larger of the two in-flight values, not the latest.
        let r1 = budget.reserve(10 * 1024 * 1024).await;
        let r2 = budget.reserve(5 * 1024 * 1024).await;
        let peak_before = budget
            .peak_current
            .load(std::sync::atomic::Ordering::Acquire);
        assert_eq!(peak_before, r1 + r2);

        budget.release(r1 + r2);
        let r3 = budget.reserve(1024 * 1024).await;
        let peak_after = budget
            .peak_current
            .load(std::sync::atomic::Ordering::Acquire);
        assert_eq!(
            peak_after, peak_before,
            "peak should not regress after releases",
        );

        budget.release(r3);
    }

    // Stress test for the CAS-based fast path: launches many concurrent
    // reservers whose combined ask fits exactly into the budget. Under
    // the previous `fetch_add → check → fetch_sub` design these would
    // spuriously inflate `current` past the cap and bounce off each
    // other. With CAS, only the actually-committed reservations move
    // `current`, so all tasks complete on the fast path with `current`
    // landing exactly at `budget`.
    #[tokio::test]
    async fn test_reserve_no_thundering_herd_under_contention() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);
        let full = budget.budget;
        let n: usize = 32;
        let per_task = full / n;
        assert!(per_task > 0, "budget too small for {n}-way contention test");

        let mut handles = Vec::with_capacity(n);
        for _ in 0..n {
            let budget = Arc::clone(&budget);
            handles.push(tokio::spawn(async move { budget.reserve(per_task).await }));
        }

        for handle in handles {
            tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                .await
                .expect("reserve hung under contention")
                .expect("task panicked");
        }

        // All `n` reservations should have committed exactly once each.
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            per_task * n,
        );
    }

    // Regression test for the lost-wakeup race in `reserve`'s wait
    // path:
    //
    //   reserve():  try_acquire    -> fail (over budget)
    //               wait_queue.push_back(notify)
    //                   ↓  ← if release runs here, must either be seen
    //                        on the second try_acquire below OR pop the
    //                        notify we just enqueued. Either path keeps
    //                        the reserver from awaiting forever.
    //               try_acquire    -> retry / await
    //               notify.notified().await
    //
    // The test deterministically opens the post-enqueue window with
    // the pause hook, fires a `release` while the reserver is parked,
    // and asserts the reserver still wakes promptly (either via the
    // recheck observing freed budget, or via the queued notify).
    #[tokio::test]
    async fn test_reserve_no_lost_wakeup_in_wait_path() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);
        let full = budget.budget;

        // Saturate the budget so the next reserve is forced into rollback.
        budget.reserve(full).await;

        // Arm pause; the spawned reserver will signal `arrived` after
        // its rollback and then wait on `resume` before enqueuing.
        let pause = budget.arm_pause_hook();

        let reserver = {
            let budget = Arc::clone(&budget);
            tokio::spawn(async move { budget.reserve(1).await })
        };

        // Wait until the reserver is parked between rollback and enqueue.
        pause.arrived.notified().await;

        // Disarm the hook so any *future* reserve call (post-fix, when
        // the reserver retries) is not also trapped.
        *budget.test_pause_hook.lock() = None;

        // Release everything. The wait queue is empty here, so
        // `wake_next` is a no-op — this is the lost-wakeup window.
        budget.release(full);

        // Let the reserver continue past the pause. With the bug it now
        // pushes onto the wait queue and awaits a notify that will
        // never come; with the fix it must observe the freed budget and
        // succeed.
        pause.resume.notify_one();

        tokio::time::timeout(std::time::Duration::from_secs(1), reserver)
            .await
            .expect("reserve hung — lost-wakeup race in rollback→enqueue gap")
            .expect("reserver task panicked");
    }

    // --- ReservationGuard --------------------------------------------------

    #[tokio::test]
    async fn test_reservation_guard_commit_records_actual() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);

        let estimated = 1000;
        let actual = 800;
        let guard = budget.reserve_guarded(estimated).await;
        guard.commit(actual);

        // After commit, current should reflect the actual decoded size,
        // not the (1.0x) reserved amount.
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            actual,
            "commit should reduce current to the actual decoded size",
        );
    }

    #[tokio::test]
    async fn test_reservation_guard_drop_refunds_reservation() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);

        let estimated = 1000;
        let multiplier_before = f64::from_bits(
            budget
                .estimate_multiplier
                .load(std::sync::atomic::Ordering::Acquire),
        );

        // Drop without commit (simulates an early-return error path).
        {
            let _guard = budget.reserve_guarded(estimated).await;
            assert_eq!(
                budget.current.load(std::sync::atomic::Ordering::Acquire),
                estimated,
            );
        }

        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            0,
            "dropped guard should refund the entire reservation",
        );

        // Multiplier must NOT shift toward zero on a refund — a failed
        // fetch observed nothing about decode ratios.
        let multiplier_after = f64::from_bits(
            budget
                .estimate_multiplier
                .load(std::sync::atomic::Ordering::Acquire),
        );
        assert!(
            (multiplier_after - multiplier_before).abs() < 1e-9,
            "guard drop must not fold a (estimated, 0) sample into the EMA \
             (before={multiplier_before}, after={multiplier_after})",
        );
    }

    #[tokio::test]
    async fn test_reservation_guard_drop_wakes_waiter() {
        let budget = Arc::new(PipelineBudget::new(0, 1));
        budget.set_multiplier(1.0);
        let full = budget.budget;

        // First reservation saturates the budget but is held by a guard.
        let blocking_guard = budget.reserve_guarded(full).await;
        assert_eq!(
            budget.current.load(std::sync::atomic::Ordering::Acquire),
            full,
        );

        // Spawn a waiter that wants 1 byte — should be parked.
        let budget_clone = Arc::clone(&budget);
        let waiter = tokio::spawn(async move { budget_clone.reserve(1).await });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!waiter.is_finished(), "second reserve should be parked");

        // Drop the guard without commit; the refund must wake the waiter.
        drop(blocking_guard);

        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("guard drop did not wake parked reserver")
            .expect("waiter task panicked");
    }

    // --- env-var parsing helpers -------------------------------------------

    const DEFAULT_BYTES: usize = 64 * 1024 * 1024;

    #[test]
    fn test_parse_bytes_accepts_iec_suffix() {
        assert_eq!(
            parse_bytes_or_default("TEST", "128MiB", DEFAULT_BYTES),
            128 * 1024 * 1024,
        );
        assert_eq!(
            parse_bytes_or_default("TEST", "1GiB", DEFAULT_BYTES),
            1024 * 1024 * 1024,
        );
        assert_eq!(
            parse_bytes_or_default("TEST", "512KiB", DEFAULT_BYTES),
            512 * 1024,
        );
    }

    #[test]
    fn test_parse_bytes_accepts_si_suffix() {
        assert_eq!(
            parse_bytes_or_default("TEST", "100MB", DEFAULT_BYTES),
            100_000_000,
        );
        assert_eq!(
            parse_bytes_or_default("TEST", "2GB", DEFAULT_BYTES),
            2_000_000_000,
        );
    }

    #[test]
    fn test_parse_bytes_accepts_bare_integer_as_bytes() {
        assert_eq!(
            parse_bytes_or_default("TEST", "67108864", DEFAULT_BYTES),
            64 * 1024 * 1024,
        );
    }

    #[test]
    fn test_parse_bytes_rejects_zero() {
        assert_eq!(
            parse_bytes_or_default("TEST", "0", DEFAULT_BYTES),
            DEFAULT_BYTES,
        );
    }

    #[test]
    fn test_parse_bytes_rejects_negative() {
        assert_eq!(
            parse_bytes_or_default("TEST", "-1", DEFAULT_BYTES),
            DEFAULT_BYTES,
        );
        assert_eq!(
            parse_bytes_or_default("TEST", "-1MB", DEFAULT_BYTES),
            DEFAULT_BYTES,
        );
    }

    #[test]
    fn test_parse_bytes_rejects_non_numeric() {
        assert_eq!(
            parse_bytes_or_default("TEST", "not-a-number", DEFAULT_BYTES),
            DEFAULT_BYTES,
        );
    }

    #[test]
    fn test_parse_bytes_rejects_unknown_suffix() {
        // Mb (megabit) is intentionally not a valid byte suffix.
        assert_eq!(
            parse_bytes_or_default("TEST", "10Mb", DEFAULT_BYTES),
            DEFAULT_BYTES,
        );
    }

    #[test]
    fn test_parse_fraction_accepts_valid_range() {
        assert!((parse_fraction_or_default("TEST", "0.5", 0.25) - 0.5).abs() < 1e-12);
        assert!((parse_fraction_or_default("TEST", "1.0", 0.25) - 1.0).abs() < 1e-12);
        assert!((parse_fraction_or_default("TEST", "0.0001", 0.25) - 0.0001).abs() < 1e-12);
    }

    #[test]
    fn test_parse_fraction_rejects_zero() {
        assert!((parse_fraction_or_default("TEST", "0.0", 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_parse_fraction_rejects_above_one() {
        assert!((parse_fraction_or_default("TEST", "1.5", 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_parse_fraction_rejects_negative() {
        assert!((parse_fraction_or_default("TEST", "-0.5", 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_parse_fraction_rejects_nan_and_inf() {
        assert!((parse_fraction_or_default("TEST", "NaN", 0.25) - 0.25).abs() < 1e-12);
        assert!((parse_fraction_or_default("TEST", "inf", 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn test_parse_fraction_rejects_non_numeric() {
        assert!((parse_fraction_or_default("TEST", "bogus", 0.25) - 0.25).abs() < 1e-12);
    }
}
