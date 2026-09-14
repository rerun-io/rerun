# Why we need `PipelineBudget`

## The problem in one sentence

When a user submits a dataframe query, our IO pipeline fetches chunks from remote storage *faster than the CPU can process them*, and without a hard ceiling on in-flight decoded bytes the process OOMs on large queries.

## Data flow

```
                                            ┌───── live in RAM ─────┐
                                            │                       │
   server          IO task            chunk_tx          CPU worker            output
  ────────       ────────────         ─────────        ─────────────         ────────

  ┌──────┐      ┌─────────┐                            ┌──────────┐
  │ S3 / │      │ fetch   │   tokio::mpsc(32)          │ insert   │  RecordBatches
  │ gRPC │ ───► │ + decode│ ────────────────►          │ Chunks   │ ───────────────►◄═══ DataFusion
  └──────┘      │   ✕24   │                            │ into     │                       poll_next()
                │ parallel│                            │ChunkStore│                       OUTPUT
                └─────────┘                            └──────────┘
                  STAGE A                                STAGE C
                                  STAGE B

                  ▲                  ▲                    ▲
                  │                  │                    │
            decoded Arrow       channel buffer       per-segment store
            up to 24 batches    bounded msg count    held until segment
            (~12 MB each)       (32 msgs)            changes, then flushed
            ≈ 288 MB             unbounded bytes     can be 100s of MB

   ▲
   │
   │
  ◄═══ DataFusion SegmentStreamExec::execute()
       INPUT (chunk_info batches: which chunks
       to fetch, sizes, URLs — comes from
       upstream catalog scan)
```

DataFusion connects at two spots only:

- **INPUT** (left) — `SegmentStreamExec::execute()` hands the IO task a `Vec<RecordBatch>` of *chunk_info* (metadata only: chunk IDs, sizes, optional direct URLs). No bulk data here.
- **OUTPUT** (right) — `DataframeSegmentStream::poll_next()` pulls finished `RecordBatch`es and yields them up the DataFusion plan. Standard `SendableRecordBatchStream` from there.

Everything between input and output runs on tokio tasks, invisible to DataFusion. Memory exists in **all three stages simultaneously**. Real RSS = A + B + C.

## What blows up

The IO task uses `futures::stream::buffer_unordered(24)` — 24 fetch+decode futures run concurrently for throughput. Each completed-but-not-yet-pulled future *holds its decoded chunks in RAM* inside the stream's internal buffer until something drains them.

If downstream blocks for any reason (slow CPU, fat segment, GC), Stage A grows to **24 × decoded-batch-size ≈ 288 MB per partition**, then a `reorder_buf` `BTreeMap` on top can hold roughly the same again. On a 6-CPU node = 6 partitions = potentially several GB before a single byte hits the channel.

That's the OOM.

## Why a byte-bounded channel (`re_quota_channel`) doesn't fix it

`re_quota_channel` puts a byte cap on **stage B only** — the channel buffer between IO and CPU. Backpressure activates when the *channel send* would exceed the cap.

Critical timing problem:

```
   fetch starts ──► bytes decoded into RAM ──► send into channel ──► recv & insert
                                  ▲                       ▲
                                  │                       │
                          bytes already exist       channel cap kicks in HERE
                          in stage A                — too late, RAM already spent
```

The channel only sees a message *after* the fetch has completed and the decoded bytes are sitting in stage A. By the time send is throttled, the 24-way concurrent fetch has already inflated RAM.

Result with a 100 MB channel cap on a 6-partition node:

| Stage | Per-partition | × 6 |
|---|---|---|
| A — buffer_unordered + reorder | ~400–600 MB | 2.4–3.6 GB |
| B — channel buffer | 100 MB | 600 MB |
| C — ChunkStore current segment | variable (S) | 6S |
| **Total** | **~500–700 MB + S** | **~3–4 GB + 6S** |

The 100 MB cap governs ~15% of the live working set. Channel-only backpressure leaves the OOM-causing stages unmetered.

## What `PipelineBudget` does differently

It's a **byte semaphore that gates entry to stage A**, not a channel:

```
   ┌──► reserve(estimated)  ─── BLOCKS HERE if budget exhausted ───
   │           │
   │           ▼
   │    fetch + decode  (stage A)
   │           │
   │           ▼
   │    send to chunk_tx  (stage B)
   │           │
   │           ▼
   │    insert into ChunkStore  (stage C)
   │           │
   │           ▼ (after segment flushed downstream)
   └────  release(bytes)
```

A reservation is held from before-the-fetch through after-the-segment-flush — **spanning stages A + B + C**. One global cap covers the entire live working set, not just one stage.

## Why the protocol can't be flattened to a channel

Three operations, three different threads, three different times:

1. **`reserve(estimated)`** — IO task, *before* fetch. Estimate from server-reported chunk size.
2. **`commit(actual)`** — IO task, *after* decode. Reconciles estimate vs measured Arrow heap size.
3. **`release(bytes)`** — CPU task, *after* `ChunkStore` flushes the segment.

A channel collapses (2) and (3) into a single `recv()`. We can't do that here:
- Bytes live in `ChunkStore` long past the recv on the IO→CPU channel — releasing on recv would understate live memory.
- Reserve must precede fetch, but channel send can only happen *with the message*, which only exists *after* fetch.

## What `PipelineBudget` adds beyond a raw semaphore

Could be built on `tokio::sync::Semaphore`. PipelineBudget adds:

- **Adaptive sizing.** Cap derived from server-reported total query size × fraction, clamped per-partition. Small queries don't waste headroom, large queries don't overcommit the host.
- **Estimate→actual EMA.** Server's wire-size is an *estimate* of decoded heap size. EMA converges on the dataset's true ratio so reservations stay accurate over the query.
- **RAII guards.** Error / early-return paths refund reservations automatically — no leak on cancellation.
- **Cross-partition shared budget.** One pool serving all partitions, vs N independent caps that don't share headroom.
- **Lifecycle telemetry.** Peak / total / release-count summary on `Drop` for tuning without `RUST_LOG=debug`.
