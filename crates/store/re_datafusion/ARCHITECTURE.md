# `re_datafusion` streaming dataset-query architecture

A dataset query can return more data than fits in memory — hundreds of
segments per partition, tens of thousands of chunks per segment — while
the consumer only needs to stream the result incrementally as
`RecordBatch`es. The streaming query path bounds per-partition working
set by emitting batches as the source chunks become safe to release,
rather than materialising the full result set before yielding.

Incremental emit is constrained by rerun's **latest-at** semantics: the
value of entity `/a` at time `T` is the most recent chunk for `/a` with
`time_min <= T`. A row at time `T` cannot be emitted until every chunk
that could affect it has arrived. The **safe horizon** is the largest
`T` such that, for every entity, every chunk with `time_min <= T` is
known to have been received. The CPU worker tracks per-segment manifests
of expected chunk start-times to compute the horizon, emits the rows
whose times fall in `(last_emitted, horizon]` as one or more
`RecordBatch`es whenever the horizon advances, and GCs chunks once they
are no longer reachable from any future row at `T <= horizon`.

The rest of the doc describes how the IO loop, CPU worker, and pipeline
budget cooperate around that horizon-driven emit so that a partition's
working set stays bounded.

> **Status:** target end-state. The streaming refactor lands incrementally;
> not every section below reflects code currently on `main`. The
> "Pre-streaming deadlock classes" appendix is the diagnostic trail back to
> `#1736` and the failure modes this design neutralises.
>
> TODO(RR-1538): drop this status note once the final PR in the streaming
> stack lands and the doc matches `main`.

## Top-level dataflow

```text
┌──────────────────────┐          ┌──────────────────────┐         ┌─────────────────────┐
│   server / network   │  fetch   │  IO loop             │   tx    │  CPU worker         │  RecordBatch
│  (gRPC + direct URL) │ ────────►│  (per-partition task)│ ──────► │  (per-partition tsk)│ ───────────►
└──────────────────────┘          └──────────────────────┘         └─────────────────────┘
        ▲                                  ▲                                  │
        │ chunk_info                       │ pipeline budget                  │
        └──────────────────────────────────┴──────────────────────────────────┘
                                  reserve / release / publish_segment_finalized
```

1. `SegmentStreamExec::execute` is the DataFusion entry point. For each output
   partition it spawns:
   - one **CPU worker** task (`chunk_store_cpu_worker_thread`)
   - one **IO loop** task (`chunk_stream_io_loop`)
   connected by a single bounded `tokio::mpsc` channel carrying `CpuWorkerMsg`.

2. The IO loop iterates the partition's `chunk_info` rows
   (already grouped by segment), groups fetches into target-sized batches
   (`create_request_batches`), and dispatches them via `buffer_unordered`.

3. Each fetch task reserves bytes from the shared `PipelineBudget` *before*
   the actual fetch starts, then commits the post-decode delta via
   `ReservationGuard::commit` and pushes decoded chunks downstream.

4. The CPU worker keys a `HashMap<SegmentId, CurrentStores>` and routes
   incoming chunks to the right per-segment in-memory `ChunkStore`. After
   every chunk insert it runs `flush_incremental` to emit any rows the safe
   horizon now allows and GC any chunks the horizon has passed.

## Per-segment lifecycle

Per segment the worker tracks:

- **`expected_chunks`** (from `CpuWorkerMsg::SegmentChunkCount`) — drives
  segment-completion detection.
- **`manifest: SegmentChunkManifest`** (from `CpuWorkerMsg::SegmentManifest`,
  built from the `{filtered_timeline}:start` columns on the `chunk_info`
  response) — drives safe-horizon computation.
- **`last_emitted_time`** — high-water mark for already-emitted rows. Used
  as `filtered_index_range.min - 1` so the next `emit_up_to` doesn't re-emit
  rows. `None` means no rows have been emitted yet.
- **`last_horizon`** — most recent value `safe_horizon` returned. Only
  consulted for the non-regression `debug_assert!`.

Lifecycle phases:

| Phase             | Trigger                                    | What runs                                                                                                              |
|-------------------|--------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| **Open**          | First `SegmentManifest` *or* `Chunks` msg  | `CurrentStores::new` builds the in-memory store + a reusable `QueryCacheHandle`.                                       |
| **Stream**        | Each `Chunks` msg                          | Insert chunks, `record_arrival` against manifest, `flush_incremental`: maybe emit + GC if horizon advanced.            |
| **Finalize**      | `received_chunks >= expected_chunks`       | Worker removes the entry, calls `flush` (drains via `emit_up_to(None)`), releases residual bytes. `Drop` is a no-op.   |
| **End-of-stream** | Channel closes with segment incomplete     | Worker logs a warning + drops the entry **without flushing**. `Drop for CurrentStores` refunds the budget reservation. |

## Non-goals

Each of these was tried in some form during early iterations and broke either
correctness (premature emit, dropped data) or the deadlock invariants. Do not
reintroduce them without first checking the listed test cases or doc sections.

### 1. Do not reintroduce the IO-side reorder buffer

Pre-streaming, `chunk_stream_io_loop` held a `BTreeMap<task_idx,
Vec<ChunksWithSegment>>` and drained it in `task_idx` order so the CPU
worker saw fetches one segment at a time. Two problems:

- **Head-of-line blocking against a full budget.** Decoded chunks sat
  in the buffer waiting on a slow predecessor by `task_idx`, even when
  the CPU worker was idle and ready to consume them. Their byte
  reservations stayed charged against the pipeline budget for the
  duration, so new IO fetches were blocked on a budget that was full
  of memory the CPU side could not yet touch — the very chunks that
  would release that budget were the ones stuck behind the buffer.
- **Unbounded amplification under bursty fetch latency.** A single
  slow predecessor pinned every later fetch in the buffer regardless
  of segment. The only cap on reorder-buffer size was the pipeline
  budget itself, and the buffer's whole effect was to keep that
  budget saturated.

The post-streaming CPU worker keys per-segment state by `SegmentId`
(`HashMap<SegmentId, CurrentStores>`) and processes whichever
segment's chunks arrive next, so the single-segment-at-a-time
invariant the reorder buffer enforced is no longer required.

### 2. Do not emit rows past the safe horizon

`flush_incremental` rebuilds the `QueryHandle` with
`filtered_index_range = (last_emitted, horizon]` on every cycle. The
upper bound is a **correctness requirement**, not a tunable: emitting a
row past the horizon under rerun's latest-at semantics means publishing
an entity's value as if it were final when a later chunk for that
entity has not yet arrived. Downstream consumers cannot distinguish a
"complete row" from an "incomplete row with carry-forward values", so
the row is silently wrong rather than visibly stale.

The same constraint motivates the end-of-stream-cleanup decision to **drop**
incomplete segments rather than flush them. The carry-forward values look
correct (non-null) but reflect data that's still in flight.

### 3. Do not drop the segment-count gate "because the byte budget is enough"

`MAX_CONCURRENT_SEGMENTS = 3` exists on top of the byte budget for two
reasons:

1. The CPU worker's per-segment HashMap grows linearly with in-flight
   segments. Without an upper bound, a long-tailed slow segment can let the
   IO side open hundreds of concurrent segments before any finalize; the
   CPU memory cost (per-segment `ChunkStore` + manifest + cache) eventually
   eclipses the byte budget's cap.
2. The per-segment manifest's `outstanding_time_mins_per_entity` map is
   `O(N_entities × N_chunks_per_entity)`. While each `flush_incremental`
   only scans one segment's manifest, the *aggregate* per-tick CPU cost
   across the worker still scales with the number of open segments times
   each manifest's size. A bounded segment count caps that aggregate.

The cap is admitted **atomically with the byte reservation** under the
`active_segments` lock in `PipelineBudget::try_admit`. Admitting only a
representative segment_id would let a multi-segment fetch from
`create_request_batches` stealth-open additional segments past the cap.

### 4. Do not add a CPU-side `publish_segment_started` path back

The `MAX_CONCURRENT_SEGMENTS` cap needs a single source of truth for
"which segments are open right now". An early iteration put that
bookkeeping on the CPU side: when the CPU worker received the first
chunk for a new segment, it called
`pipeline_budget.publish_segment_started(segment_id)` to add the
segment to the budget's `active_segments` set. This felt natural
because the CPU worker already keys `CurrentStores` by segment — it
has the authoritative view of which segments are being actively
worked on.

It does not work because admission has to be atomic with the byte
reservation. Concrete failure:

1. IO loop has segments `K-2`, `K-1`, `K` open (cap = 3) and is
   about to fetch the first chunk batch for segment `K+1`.
2. IO calls `reserve(bytes)` for `K+1`. The budget's segment-gate
   doesn't yet know about `K+1` — no chunk has reached the CPU side,
   so `active_segments` still shows 3 — and the check passes. Bytes
   get reserved.
3. While that fetch is in flight, IO pulls the next batch: first
   fetch for `K+2`. Same check, same outcome.
4. By the time the CPU worker eventually receives the first chunk for
   `K+1` and calls `publish_segment_started`, IO has already
   over-reserved bytes for multiple segments past the cap. The cap
   is now advisory; the per-segment HashMap + manifest cost the cap
   exists to bound can blow up unboundedly.

The fix is to fire the signal on the IO side, atomically with the
byte reservation, under the same `active_segments` lock that admits
the bytes. `PipelineBudget::try_admit` takes `(segment_ids, bytes)`
together: either both gates clear and the slots + bytes are taken,
or neither is and the caller parks. The IO side already knows which
segments a batch covers (from `create_request_batches`), so it has
everything it needs at admission time.

`publish_segment_finalized` is the symmetric counterpart but lives on
the *CPU* side, and that asymmetry is deliberate. Finalization
*frees* a slot rather than taking one, so eventually-consistent
signalling is safe: a brief over-count of "open" segments slightly
reduces concurrency but never violates the cap. The CPU side is also
where finalization is observable — it depends on the horizon emit
draining the segment and downstream consumer rate, neither of which
the IO loop sees — so the signal naturally lives in
`Drop for CurrentStores`.

## Budget gating: byte budget + segment count + stall-breaker

`PipelineBudget::try_admit` is the single atomic-admission point for IO
fetches. It must clear three gates (or trip the stall escape):

```text
                 ┌───────────────────────────────────────────────┐
reserve ────────►│ force_overcommit set?  yes → admit everything │
                 │                         no  ↓                 │
                 │ active_segments + new_segments > MAX?  yes → park
                 │                                         no  ↓ │
                 │ try_acquire(bytes)                            │
                 │   - CAS on `current` against `budget`         │
                 │   - fail → park; success → admit              │
                 └───────────────────────────────────────────────┘
                                                                 │
                                  ┌──────────────────────────────┴───┐
                                  │ wait_queue: BinaryHeap<Reverse<  │
                                  │   PriorityWaiter { task_time_min,│
                                  │                    seq, notify } │
                                  │ >>>                              │
                                  │ → earliest-time wakes first      │
                                  └──────────────────────────────────┘
```

Wake sources:
- `release(bytes)` — CPU side returns decoded bytes (incremental from
  `gc_up_to_horizon` or final from `flush`)
- `adjust_reservation(estimated, reserved, actual)` — IO side after decode
  if `actual < reserved`
- `publish_segment_finalized(segment_id)` — CPU side `Drop for CurrentStores`
- `notify_empty_emit` crossing `STALL_EMPTY_EMIT_THRESHOLD` with budget at
  `STALL_SATURATION_THRESHOLD` → sets `force_overcommit` and wakes one

Resets of `force_overcommit`:
- Any `release` (real byte progress)
- `notify_row_emitted` (CPU side emitted rows)

## Carry-forward protection in GC

`gc_up_to_horizon` looks correct under range-based intuition: "drop chunks
with `time_max < horizon`". That is **wrong** under rerun's latest-at
semantics.

Example: entity `/a` has its only chunk at `t=10`; entity `/b` has chunks
at `t=20` and `t=40`. After `/b@20` arrives, the safe horizon is `39`
(B's earliest unreceived is 40, minus 1). Dropping `/a@10` because its
`time_max < 39` makes every row at time `>= 10` emit `/a` as null instead
of carrying `/a@10`'s value forward.

Two protections, applied via `ChunkStore::gc` options:

- `protected_chunks` — union of `latest_at_relevant_chunks_for_all_components`
  over every entity in the store, evaluated at the horizon. Includes static
  chunks. This is the set of chunks that any future row at `T <= horizon`
  could resolve to under latest-at.
- `protected_time_ranges` — `(horizon+1, +inf]` on the filtered timeline.
  Anything past the horizon is by definition unread.

The intersection of these is "fair game for GC" — typically the bulk of a
segment's already-emitted chunk store, since latest-at usually resolves to
a small set of recent chunks per entity.

## Manifest/chunk divergence

`SegmentChunkManifest::record_arrival` returns `bool` (`#[must_use]`). The
manifest is built from the server's `chunk_info` rows; the CPU side then
sees the *actual* chunks from the fetch path. Divergence between the two
can happen when:

- The server's `chunk_info` and chunk fetch responses are out of sync
  (deploy in progress, stale cache).
- A chunk's `time_min` on the filtered timeline doesn't match the
  `{timeline}:start` value the server announced (bug in chunk encoding
  or split logic).
- The CPU side's `:start` extraction missed a row (bug in
  `build_segment_manifests`).

Divergence is **silent data loss** if not surfaced: the chunk inserts into
the store, but if `safe_horizon` has already advanced past the chunk's
`time_min` (because the manifest didn't know to gate on it), the row range
filter `(last_emitted, horizon]` excludes the chunk's rows entirely and
they never emit.

`record_arrival`'s contract: `false` return means "this didn't fit the
expected set" and the worker fires `re_log::debug_panic!` + `re_log::error_once!`.
The chunk still inserts — dropping it would be worse — but the log surfaces
the problem.

## Budget hazards the design must keep closed

The byte budget deadlocks whenever `release` blocks on the same chunks
that `reserve` has parked new fetches behind: if no segment can make
progress because its remaining chunks are stuck at `reserve`, and the
budget can only refund via segment completion, the pipeline transitions
from network-rate-limited to release-rate-limited and throughput goes
to zero. The post-streaming design avoids this invariant by releasing
per-chunk as the safe horizon advances rather than once per segment,
so the release rate is bounded below by the chunk-arrival rate even
under saturation.

The classes below are the deadlock-shaped (and one near-miss)
pressures the design absorbs. They are the load-bearing reason each
mechanism exists; review them before relaxing or removing any of
those mechanisms.

#### A — sizing trips on small datasets

`total_uncompressed × fraction / num_partitions` can fall below
`MIN_BUDGET_PER_PARTITION` on small datasets, so the clamp dominates
and the effective budget is `MIN × N` regardless of the real working
set. Without per-chunk release this deadlocks any partition whose
segment working set exceeds `MIN`. Per-chunk release pins the working
set to `~MAX_CONCURRENT_SEGMENTS × bytes_per_chunk`, well below the
64 MiB floor for typical chunks. The stall-breaker is the safety net
for any residual pathological case.

#### B — operator sets cap too low via env

`RERUN_PIPELINE_BUDGET_MAX=128MiB` or similar below observed peak
working set. Same code path as A, human-driven. Same resolution.

#### C — many partitions × wide datasets

Working set per partition is `~MAX_CONCURRENT_SEGMENTS ×
bytes_per_chunk` regardless of schema width. Wide datasets produce
smaller chunks more frequently rather than blowing the budget.

#### D — out-of-order segment chunk arrival

Two mechanisms cooperate:

- Priority-wake on `task_time_min`: the horizon-advancing chunk
  (smallest time) is woken first when the budget frees, so it
  preempts later-time chunks contending for the same slot.
- Stall-breaker: if `STALL_EMPTY_EMIT_THRESHOLD` consecutive
  `flush_incremental` calls emit zero rows while the budget is
  `STALL_SATURATION_THRESHOLD`-saturated, `force_overcommit` bypasses
  both gates so the parked horizon-advancing chunk admits, lands on
  the CPU side, advances the horizon, and pays back the overcommit
  via the subsequent real release.

#### E — CPU-worker error before `release`

`flush(...).await?` propagates `Err`. Without an RAII guard, the `?`
returns from `chunk_store_cpu_worker_thread` before the matching
`release` runs, pinning the partition's bytes for the rest of the
query and starving sibling partitions of the same query. Same hazard
if a panic unwinds past the release line.

`Drop for CurrentStores` releases `store_bytes()` whenever the
`released` flag is `false`. The explicit success path inside `flush`
sets the flag before drop runs, so the refund happens exactly once.
Any `?`, panic, or cancellation path produces the refund via `Drop`.
Unit test: `test_current_stores_drop_refunds_budget`.

#### F — stream cancellation mid-segment

Consumer hangs up (LIMIT, plan cancellation). The CPU worker's
`CurrentStores` is dropped before `flush` completes. `Drop for
CurrentStores` covers this path identically to E: `released == false`,
refund fires.

#### G — EMA over-estimation

After several large-expansion samples, `estimate_multiplier` climbs
toward `MAX_ESTIMATE_MULTIPLIER`. Subsequent reservations =
`estimated × multiplier` even when true expansion has settled back
to ~1.0. The `MAX_ESTIMATE_MULTIPLIER` clamp keeps this bounded; the
EMA smoothing factor decays the influence of outliers.

#### H — single fetch larger than entire budget — not a deadlock

`reserved > budget` path bypasses the gate with a warn-level log.
`current` over-commits by that fetch. No deadlock; documented edge
case.

## Compile-time defaults that are load-bearing

| Default                       | Value         | Why this number                                                                                                |
|-------------------------------|---------------|----------------------------------------------------------------------------------------------------------------|
| `BUDGET_FRACTION`             | `0.25`        | IO may run at most one quarter of the query's total decoded estimate ahead of the CPU side.                    |
| `MIN_BUDGET_PER_PARTITION`    | `64 MiB`      | Enough headroom for `MAX_CONCURRENT_SEGMENTS * bytes_per_chunk` on realistic workloads.                        |
| `MAX_BUDGET_PER_PARTITION`    | `1 GiB`       | Caps worst-case per-partition RSS for large datasets.                                                          |
| `MAX_CONCURRENT_SEGMENTS`     | `3`           | Bounds the CPU worker's HashMap size + the `O(N_open_segments)` horizon-recompute work.                        |
| `INITIAL_ESTIMATE_MULTIPLIER` | `1.5`         | Cold-start over-reserve by ~50% so first few fetches don't transiently OOM if expansion is higher than typical.|
| `ESTIMATE_EMA_ALPHA`          | `0.2`         | EMA convergence within a handful of samples while tolerating one-off outliers.                                 |
| `STALL_EMPTY_EMIT_THRESHOLD`  | `20`          | High enough that "horizon hasn't moved yet" stalls don't trip; low enough that a real deadlock breaks fast.    |
| `STALL_SATURATION_THRESHOLD`  | `0.95`        | A query waiting on slow IO with budget mostly empty shouldn't falsely trigger the bypass.                      |
| `FLUSH_BATCH_ROWS`            | `2048`        | Inherited from non-streaming path (`#1794` / `#1822`).                                                         |
| `FLUSH_BATCH_BYTES`           | `200 MiB`     | Same.                                                                                                          |

All bytes-sized constants are overridable at runtime via `RERUN_PIPELINE_BUDGET_*`
env vars. The thresholds and counts are not, by design — they're chosen for
the architectural invariants above rather than per-deployment tuning.
