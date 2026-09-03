# Streaming dataset-query pipeline v2 — design proposal

> **Status:** proposal, not yet implemented.
> This document records the architecture investigation of 2026-08 and the resulting design ("B′").
> `ARCHITECTURE.md` and `PIPELINE_BUDGET.md` describe the *current* implementation; this document describes its intended replacement.

## Why revisit this

The current pipeline works, but it has grown into a shape that is hard to maintain and hard to reason about:

- Two cooperating tasks per partition (IO loop + CPU worker) on different runtimes, coupled through 7 distinct coordination mechanisms, 4 message shapes, and 7 budget verbs.
- ~11 distinct cleanup/cancellation paths.
- A segment can live in 3 different CPU-worker structures (`current_stores`, `ready_pending`, `completed_segments`), indexed by a fourth (`emit_order`).
- A stall-breaker escape hatch (`force_overcommit`) whose job is to un-wedge deadlocks the design itself makes possible.
- The byte budget ships with defaults deliberately chosen to never bite (`FRACTION=1.0`, `MIN=4 GiB`), so the segment-count gate is the only live throttle — most of `PipelineBudget` guards paths that do not run in production.

The commit history shows the cost: a 6-part streaming refactor followed by a steady stream of corrective patches (segment-gate releases, query-scoped admission, adaptive admission, fetch-concurrency caps).

## Requirements

1. **Correctness under latest-at semantics.**
   A row at time `T` may only be emitted once every chunk with `time_min <= T` for that segment has arrived (the *safe horizon*).
   Emitting early produces silently wrong carry-forward values.
2. **Bounded memory / no client OOM**, including larger-than-memory datasets and larger-than-memory single segments.
3. **Throughput.**
   Many concurrent `FetchChunks` requests, and — critically — **cross-segment fetch coalescing**: packing many small chunks (possibly from many small segments) into few large requests has been measured to make order-of-magnitude differences in query time.
4. **Fast small queries.**
   Heavily filtered queries returning many tiny segments must not be throttled by machinery sized for the huge case.
5. **CPU-bound work must not block the IO path.**
6. **Maintainability.**

## Findings that shaped the design

### The output-ordering claim is worth keeping

`SegmentStreamExec` advertises per-partition ordering `[rerun_segment_id ASC, <sort_index> ASC]` plus `Partitioning::Hash([rerun_segment_id], n)`.
Analysis of DataFusion 54 planning plus a survey of every in-tree consumer showed:

- **`ORDER BY segment_id[, time]`** plans as a streaming `SortPreservingMergeExec` (~1 batch per partition, first row after first segment) instead of a full materializing sort (O(result) RAM or disk spill, first row after full scan).
  With `LIMIT k` it additionally early-exits and cancels upstream fetches instead of downloading the whole dataset.
  Hash partitioning is no obstacle: SPM only needs each input stream individually sorted.
- **Window functions `PARTITION BY rerun_segment_id ORDER BY time`** plan as `BoundedWindowAggExec(Sorted)` with bounded frame state instead of a per-partition full sort.
  This is the flagship documented pattern (`dataframe_operations.py` sub-episode detection, the `llm.ipynb` notebook) and has dedicated A/B benchmarks in `component-tests` (`light_window_with_ordering` vs `light_window_requires_reordering`, `droid.py::segment_time_ordering`).
- `GROUP BY segment_id` / `DISTINCT` gains are modest (group cardinality is small); joins benefit from the Hash *partitioning* claim, not the ordering.
- No in-tree consumer reads global row order positionally; every order-sensitive consumer applies its own `.sort()` — which the claim turns from a spill-sort into a streaming merge.
- **A violated claim is silently wrong, not slow.**
  SPM does not validate input sortedness, and sorted-mode aggregation emits duplicate group rows if a key reappears.
  User `.sort()` calls are elided *because* of the claim, so broken emission order propagates silently.
  The claim and the ordered-emit enforcement are a matched pair: keep both or drop both.
- The strict `segment_id ASC` key is stronger than the valuable workloads need — windows and streaming group-bys only need "segments contiguous, time-ordered within segment" — but DataFusion 54 has no "clustered but unordered" property, so strict ASC is the price of the grouping claim.
  A clustered-ordering property upstream in DataFusion is the long-term escape valve if ordered emission ever becomes the binding constraint.

**Decision: keep the full two-column claim.** The v2 design makes it cheap (see below).

### The safe horizon is a watermark

The per-entity `SegmentChunkManifest` computes: min over entities of the earliest outstanding chunk `time_min`.
That is identical to the *global* min `time_min` over outstanding chunks of the segment — the per-entity structure tracks disorder that the client itself creates by fetching with `buffer_unordered`.
If chunk deliveries arrive in per-segment `time_min` order, the horizon is simply "the `time_min` of the next undelivered chunk, minus one" — one plan cursor per segment, no manifest, no manifest message protocol, no manifest-before-chunks ordering hazard.

### Segments are data-independent

Latest-at carry-forward never crosses segments.
One `ChunkStore` + `QueryEngine` per segment is already the unit of work; nothing about correctness requires segments to share a worker.

### Concern separation (the core simplification)

The current design fuses four concerns into one mechanism (`PipelineBudget` + gates + horizon + stall-breaker).
Each has a simpler canonical owner:

| Concern | Current owner | v2 owner |
|---|---|---|
| Runtime isolation | IO loop on ambient rt, CPU worker on shared rt, message protocol between | Whole pipeline on the CPU runtime; each network call spawned onto the IO runtime; one bridge to DataFusion (the IOx `DedicatedExecutor` inversion) |
| Pacing / backpressure | Byte semaphore + segment gate + priority wake + stall breaker | Pull-based streams; backpressure = "don't poll"; bounded concurrent fetches |
| Memory ceiling | Same byte semaphore (currently inert) | One issuance window `W` (plain semaphore) + optional DataFusion `MemoryReservation` for the query-wide hard limit |
| Emit correctness (horizon) | Per-entity manifest + protocol messages | Per-segment plan cursor (watermark) |

## Options considered

| | Option | Summary |
|---|---|---|
| A | Tune status quo | Engage the byte budget with intended values, keep the architecture |
| B | Per-segment owned pipeline | One async stream per segment owning fetch → store → emit → GC; ordered concat across segments |
| **B′** | **B + shared coalescing fetch layer** | **B with fetch ownership moved to a plan-driven executor that coalesces requests across segments — the selected design** |
| C | B without the ordering claim | `flatten_unordered` across segments; consumers pay real sorts |
| D | Spill hybrid | B/C plus divert compressed chunks to disk under memory pressure |
| E | Storeless k-way merge | Replace `ChunkStore`/`QueryHandle` with direct cursor merge |
| F | DataFusion-native decomposition | Lean on engine primitives (`RecordBatchReceiverStreamBuilder`, `MemoryPool`, SPM) |

Decision matrix (✅ good / 🟡 workable / ❌ poor):

| Criterion | A | B/B′ | C | D | E | F |
|---|---|---|---|---|---|---|
| Latest-at correctness confidence | ✅ proven | ✅ same store/emit path | ✅ | ✅ | ❌ reimplements semantics | ✅ |
| No-OOM guarantee | 🟡 budget inert | ✅ structural + pool | ✅ | ✅✅ | ✅ | ✅ |
| Larger-than-memory segment | 🟡 | ✅ | ✅ | ✅ | ✅ | ✅ |
| Many-tiny-segments latency | 🟡 | ✅ | ✅✅ | ✅ | ✅ | ✅ |
| Peak throughput | 🟡 | ✅ | ✅✅ | ✅ | ✅ | ✅ |
| Cross-segment fetch coalescing | ✅ | ✅ (B′) | ✅ (B′) | ✅ | — | — |
| Ordering claim preserved | ✅ | ✅ | ❌ | inherits | ✅ | ✅ |
| Complexity / maintainability | ❌ | ✅ | ✅✅ | 🟡 two paths | ❌ | ✅ |
| Migration risk | ✅ none | 🟡 rewrite of io_loop + cpu_worker; store/emit/GC ports as-is | 🟡 | ❌ | ❌ | ✅ piecemeal |

**Selected: B′**, borrowing F's primitives.
E is rejected outright: the `QueryHandle` surface (per-ancestor clears, static chunks, overlaps, sparse fill) is deep and battle-tested; reimplementing it is where the correctness risk lives.
D is deferred until there is evidence of skew that structural bounds cannot hold.
C remains available as a degenerate configuration (the claim is already conditional on the projection).

## The v2 design

Per DataFusion partition, one owned stream tree.
The whole tree runs on the process-global CPU runtime; network calls are spawned onto the IO runtime and awaited (the inversion of today's split — isolation without a coordination protocol).
One bridge to DataFusion at the top, as today.

```text
chunk_infos (known up front, per partition)
   │
   ▼
1. Fetch planner  — today's create_request_batches logic, unchanged:
   pack chunks across segments into ~8 MiB requests, ordered by
   [global segment order, time_min within segment].
   Output: Vec<PlannedBatch>, immutable, shared.
   │
   ▼
2. Fetch executor — stream::iter(batches)
                    → map(|b| spawn fetch+decode on IO runtime)
                    → buffered(N)
   N concurrent RPCs; gRPC request grouping and the direct-URL
   concurrency semaphore live here, unchanged.
   Issuance gated by the window W (below).
   │
   ▼
3. Router — chunk → segment is known from the plan;
   push each decoded chunk to its segment's queue.
   │
   ▼
4. Per-segment processing streams (one per segment, lazy):
   insert into per-segment ChunkStore (config ALL_DISABLED, as today)
   → emit rows in window (processed, horizon] via a fresh QueryHandle
   → GC to horizon with carry-forward protection (as today)
   │
   ▼
5. Ordered concat across segment streams, in global segment order
   (this is what keeps the [segment_id ASC, sort_index ASC] claim honest)
   │
   ▼
DataFusion (SendableRecordBatchStream)
```

### The two decisions that make it simple

**1. Issuance order = emission order.**
The planner emits batches in the same global segment order (and `time_min` order within a segment) that the output stream emits rows.
This kills the historical deadlock class at the root: the oldest outstanding fetch always belongs to the segment the consumer needs next, so window releases always flow.
The priority-wake queue, the segment-count gate, and the stall breaker all existed to arbitrate between completion-order issuance and emission-order release; that divergence no longer exists, so none of them are needed.

**2. One flow-control knob: the issuance window `W`.**
The executor may hold at most `W` estimated-decoded bytes that have been issued but not yet emitted/GC'd.
Acquire on issue (using `chunk_byte_size_uncompressed` where present), release on emit/GC/segment-drop via an RAII guard.
Clamp so at least one batch is always admissible (the moral equivalent of today's hazard-H bypass, and of "every channel has ≥ 1 buffer" in credit-based flow control).
Because of decision 1, `W` cannot wedge.
Per-segment queues need no capacity tuning — total buffered bytes are bounded by `W` by construction.

Properties that fall out of `W` alone:

- **Tiny segments:** `W` spans many whole segments → deep cross-segment coalescing and lookahead.
  This *replaces* adaptive segment admission (#2912): byte-measured lookahead adapts automatically.
- **Huge segments:** `W` lies mostly inside the head segment → the head streams larger-than-memory via horizon emit + GC, while prefetch of successor segments is bounded.
- Optionally register `W` as a DataFusion `MemoryReservation` so the query participates in the engine-wide memory limit and fails fast instead of OOMing in a pathological case.
  Note: the DataFusion pool cannot *wait* — it is the ceiling, never the pacing mechanism.

### Watermark (safe horizon)

With the executor using ordered `buffered(N)`, deliveries arrive in plan order, so each segment's horizon is a **plan cursor**: the `time_min` of its next undelivered chunk, minus one.
No manifest structure, no manifest messages, no per-entity tracking.
The plan is immutable shared data available before anything runs, so the "all-or-nothing manifest build" rule and the manifest-before-chunks ordering hazard both disappear by construction.

`buffered(N)` vs `buffer_unordered(N)`:

- **`buffered(N)` (default):** delivery in plan order while N RPCs run concurrently.
  Cost: a straggler RPC delays *delivery* (not execution) of up to N−1 completed batches — bounded by one straggler per window.
- **`buffer_unordered(N)` (fallback):** maximum delivery throughput; the watermark then needs a small per-segment outstanding-min structure (`BTreeMap<TimeInt, count>` seeded from the plan — plain local data, not a protocol).
  Swap is localized to the executor + horizon computation; adopt only if benchmarks show straggler variance matters.

### Ordering and head-of-line cost

Ordered concat means only the head segment emits; successor segments buffer.
Unlike today's `ready_pending` (which accumulates entire decoded segments bounded only by budget backpressure), a successor segment's buffering is bounded by its share of `W`.
The residual cost is a pipeline bubble at segment switch: when the head finishes, its successor has only its `W`-share prefetched.
This is tunable via `W` and measurable with the existing `performance.py` ordering A/B benchmarks.

### Cancellation and errors

The whole tree is one owned stream: dropping it (LIMIT, plan cancellation, consumer hangup) cancels the executor, the in-flight spawned fetches, and every segment stream; `W` is refunded by guard `Drop`.
One cleanup path replaces today's ~11.
Errors propagate in-band through the stream (no join-handle harvesting in `poll_next`).
Incomplete segments at end-of-stream are still dropped, not flushed — partial latest-at output is silently wrong (unchanged rule).

### What is built, kept, deleted

| Build (new, small) | Keep (unchanged) | Delete |
|---|---|---|
| Router (plan lookup + queue push) | Fetch planning (`create_request_batches` packing logic) | `CpuWorkerMsg` protocol + IO→CPU channel choreography |
| `W` window semaphore + RAII guard | Transport (`chunk_fetcher.rs`, gRPC grouping, direct-URL path + its semaphore) | `PipelineBudget` (byte CAS, priority heap, segment gate, EMA estimator, stall breaker) |
| Per-segment stream driver (wraps existing insert/emit/GC fns) | Store insert (`ALL_DISABLED`), `QueryHandle` windowed emit, carry-forward-protected GC — byte-for-byte | `SegmentChunkManifest` module + its messages |
| Ordered-concat combinator over segment streams | Ordering claim + `test_segment_ordering` | `emit_order` / `ready_pending` / `completed_segments` bookkeeping |
| Optional `MemoryReservation` registration | Process-global CPU runtime, `query_dataset` fan-out + semaphore | Adaptive segment admission (subsumed by `W`) |

### Guardrails carried over

- **Estimate sanity:** `W` acquisition uses `chunk_byte_size_uncompressed` when present; a single batch larger than `W` is admitted with a warning (never deadlocks).
  Whether the estimate→actual EMA is still needed should be decided by measurement; expansion is typically ~1.0 and `W` is a soft pacing bound, not a hard ceiling.
- **`using_index_values` mode:** incremental range-window emit would replay rows (the expression overrides `filtered_index_range` inside `QueryHandle`).
  In v2 this is a *local* mode of one segment stream — buffer the whole segment, emit once at completion — instead of a global worker mode.
- **No emit past the horizon** and **drop incomplete segments at EOS**: unchanged correctness rules; see `ARCHITECTURE.md` non-goal #2.
- **`LatestAtCache` staleness** remains safe only under horizon monotonicity — the v2 plan-cursor horizon is monotone by construction, but keep the non-regression `debug_assert`.

### Pre-existing issues to fix alongside (independent of the redesign)

- **Fixed:** the ordering and Hash claims hardcoded `Column::new(RERUN_SEGMENT_ID, 0)`.
  Both now resolve the index by name against the projected schema, like the `sort_index` lookup already did.
- **Fixed:** the wasm provider (`dataframe_query_provider_wasm.rs`) declared the ordering unconditionally, without the projection guard the native path has.
  Both providers now go through `segment_stream_plan_properties`, which carries the guard.
- **Partly fixed:** `local_chunk_store_provider.rs` declared no ordering although its output is time-sorted, so local-path queries paid sorts the remote path skips.
  The `[<filtered_index> ASC]` claim is now declared, but `re_sorbet` declares index columns nullable and DataFusion demands an exact `nulls_first` match on nullable sort expressions, so the claim only elides the sort for `NULLS FIRST` consumers.
  SQL's bare `ORDER BY <index>` is `NULLS LAST` and still pays a full sort.
  See the blocked-work note on `segment_stream_plan_properties` in `dataframe_query_common/segment_stream_common.rs` for why declaring the field non-nullable is not yet an option.

## Open questions

1. **`N` (concurrent RPCs) and `W` (issuance window) defaults.**
   Two knobs replace ~10 constants; the segment-switch bubble measurement decides `W`, and existing fetch-throughput benchmarks decide `N`.
2. **`buffered` vs `buffer_unordered`** in the executor (see watermark section) — benchmark straggler sensitivity on a high-latency-variance store.
3. **Coalescing across DataFusion partitions.**
   Plans are per-partition (segments hash-partitioned), so requests never coalesce across partitions.
   Today has the same property; if cross-partition coalescing is ever wanted, the executor could be lifted to a per-query service, at the cost of re-introducing cross-task routing.
4. **Upstream DataFusion "clustered" ordering property** — would remove ordered concat entirely with zero loss of the window/aggregate benefits; worth tracking as a contribution.

## Migration sketch

1. Land the pre-existing fixes (column-index lookup, wasm guard) — independent, low risk.
2. Build the per-segment stream driver against the existing insert/emit/GC functions; unit-test against `LocalChunkStoreTableProvider`-style single-segment cases.
3. Build planner-executor-router + `W`; wire the tree behind a feature flag or env switch alongside the current path.
4. Validate with the existing e2e suites (`test_segment_ordering`, pushdown/filter suites) and the `component-tests` performance pairs (ordering A/B, coalescing-sensitive small-chunk datasets, larger-than-memory segment).
5. Cut over; delete `pipeline_budget.rs`, `segment_chunk_manifest.rs`, `io_loop.rs`, `cpu_worker.rs`; fold what this document describes into `ARCHITECTURE.md` and retire `PIPELINE_BUDGET.md`.
