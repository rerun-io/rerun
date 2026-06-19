---
title: RRD format
order: 725
---

An RRD is the file format Rerun uses to persist recordings and blueprints. At the lowest level it is a linear sequence of framed messages — store announcements and chunks of data — optionally followed by a footer index that makes random access cheap. This page covers the envelope around chunks and how they are serialized; the chunk data model itself is described in [Chunks](chunks.md).

## Stores

Logical groupings of chunks form so-called stores.
They come in two flavors: [recording](recordings.md) and [blueprint](../visualization/blueprints.md).
Both are structurally identical and distinguished only by a flag (store kind).

A single RRD can hold any number of stores.
The file extension is either `.rrd` or `.rbl`.
Both refer to the exact same on-disk format and are used conventionally:
- `.rrd` files hold any combination of recording and blueprint stores;
- `.rbl` files hold a single blueprint store.


## Message kinds (`LogMsg`)

The body of an RRD is a sequence of `LogMsg`s. There are three variants:

- **`SetStoreInfo`** announces a new store and carries its [`StoreInfo`](#store-metadata-storeinfo).
  It must appear before any data for that store.
  There can be more than one `SetStoreInfo` for the same store in a single stream — for example, when a `RecordingStream` is created and later attached to a `FileSink` — and the latest one wins.
- **`ArrowMsg`** carries the actual data: an [Apache Arrow IPC](https://arrow.apache.org/docs/format/Columnar.html#ipc-streaming-format) payload encoding a single chunk, tagged with the `StoreId` it belongs to.
  This is what makes up the bulk of every RRD.
- **`BlueprintActivationCommand`** is the only non-data control message.
  It is emitted after a blueprint's chunks have been sent, and lets the producer atomically activate the blueprint via the [`make_active` / `make_default`](https://ref.rerun.io/docs/python/stable/blueprint/) flags.
  It exists so that the Viewer never sees a half-loaded blueprint, and so the application can decide whether to apply the blueprint as the current one or the default.


> [!NOTE]
> At the wire level there is also an `End` message kind that frames the optional footer described [below](#footer). It is not a `LogMsg` variant in the application-level type system — it is an envelope reserved for the footer payload — but it shares the same framing as the three `LogMsg`s above.


## Chunks (`ArrowMsg` payload)

Every `ArrowMsg` carries a single **chunk** — an Apache Arrow `RecordBatch` with Rerun-specific schema metadata. A chunk belongs to one entity path and holds a contiguous run of rows for that entity, with one column per timeline and one column per component. See [Chunks](chunks.md) for the conceptual deep-dive (how chunks are built, batched, sorted, compacted); this section just shows what a chunk looks like when you crack one open.

The schema is laid out per **Sorbet**, Rerun's object-model spec — it defines how chunks, archetypes, components, and timelines map onto Arrow column names, types, and metadata. The easiest way to see it concretely is to save a recording and reopen it with [`RrdReader`](https://ref.rerun.io/docs/python/stable/experimental/#rerun.experimental.RrdReader).

First, let's create an RRD file with some content:

snippet: concepts/rrd_format[write]

Then we can inspect the first chunk it contains:

snippet: concepts/rrd_format[inspect]

> [!NOTE]
> By default, `chunk.format()` trims metadata keys to keep the representation concise.
> Using `trim_metadata_keys=False` disables this behavior, so the typical `rerun:` / `sorbet:` prefixes are visible here.

This prints a chunk together with its schema. A typical output looks like:

```text
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                                            │
│ * rerun:entity_path: /points                                                                                                                                                                         │
│ * rerun:id: chunk_18B0AA9FA7B7B1A61d23c55ca87b18b4                                                                                                                                                   │
│ * sorbet:version: 0.1.3                                                                                                                                                                              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────────────────────┬─────────────────────────┬────────────────────────────┬────────────────────────────┬──────────────────────────────────┬─────────────────────────────────────┐ │
│ │ RowId                               ┆ frame                   ┆ log_tick                   ┆ log_time                   ┆ Points3D:colors                  ┆ Points3D:positions                  │ │
│ │ ---                                 ┆ ---                     ┆ ---                        ┆ ---                        ┆ ---                              ┆ ---                                 │ │
│ │ type: non-null FixedSizeBinary(16)  ┆ type: Int64             ┆ type: Int64                ┆ type: Timestamp(ns)        ┆ type: List(UInt32)               ┆ type: List(FixedSizeList(3 x        │ │
│ │ ARROW:extension:metadata:           ┆ rerun:index_name: frame ┆ rerun:index_name: log_tick ┆ rerun:index_name: log_time ┆ rerun:archetype: Points3D        ┆ non-null Float32))                  │ │
│ │ {"namespace":"row"}                 ┆ rerun:is_sorted: true   ┆ rerun:is_sorted: true      ┆ rerun:is_sorted: true      ┆ rerun:component: Points3D:colors ┆ rerun:archetype: Points3D           │ │
│ │ ARROW:extension:name: TUID          ┆ rerun:kind: index       ┆ rerun:kind: index          ┆ rerun:kind: index          ┆ rerun:component_type: Color      ┆ rerun:component: Points3D:positions │ │
│ │ rerun:is_sorted: true               ┆                         ┆                            ┆                            ┆ rerun:kind: data                 ┆ rerun:component_type: Position3D    │ │
│ │ rerun:kind: control                 ┆                         ┆                            ┆                            ┆                                  ┆ rerun:kind: data                    │ │
│ ╞═════════════════════════════════════╪═════════════════════════╪════════════════════════════╪════════════════════════════╪══════════════════════════════════╪═════════════════════════════════════╡ │
│ │ row_18B0AA9FA79D51886952b7c6bb9f6ed ┆ 0                       ┆ 0                          ┆ 2026-05-18T13:04:15.500740 ┆ [4278190335, 16711935]           ┆ [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]  │ │
│ │ 4                                   ┆                         ┆                            ┆                            ┆                                  ┆                                     │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_18B0AA9FA7B6D7E06952b7c6bb9f6ed ┆ 1                       ┆ 1                          ┆ 2026-05-18T13:04:15.501658 ┆ [65535]                          ┆ [[2.0, 2.0, 2.0]]                   │ │
│ │ 5                                   ┆                         ┆                            ┆                            ┆                                  ┆                                     │ │
│ └─────────────────────────────────────┴─────────────────────────┴────────────────────────────┴────────────────────────────┴──────────────────────────────────┴─────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

What to notice:

- All Rerun-specific metadata keys are prefixed with `rerun:` (`rerun:entity_path`, `rerun:id`, `rerun:kind`, `rerun:index_name`, …). Sorbet's own metadata uses the `sorbet:` prefix (`sorbet:version`).
- The chunk-level **metadata** identifies the entity path the chunk belongs to and the chunk's id.
- The **`RowId`** column is the row identity column (`rerun:kind: control`).
- Each timeline contributes one **index column** (`frame`, `log_tick`, `log_time`) — `log_time` is auto-populated by the logging API (and `log_tick` if opted in), `frame` is the user-defined timeline.
- Each component contributes one **data column** (`Points3D:colors`, `Points3D:positions`) carrying the per-row values.


## Store metadata (`StoreInfo`)

Every store in an RRD is identified by a `StoreId` and described by a `StoreInfo`:

- **`StoreId`** combines:
    - **`kind`** — `Recording` or `Blueprint`. The on-disk format treats both identically; the kind is just a flag. What differs is the *expected content*: recordings hold user-logged data on user-defined entity paths, blueprints hold `rr.blueprint.*` objects on Viewer-reserved paths. The Viewer dispatches on the kind — recordings populate the data store, blueprints populate the Viewer's UI/layout state.
    - **`application_id`** — a user-chosen identifier for the application that produced the recording (see [Recordings](recordings.md) for the conventions, including the relationship with segment and dataset IDs in the remote/catalog context).
    - **`recording_id`** — a UUID or user-chosen string that distinguishes runs of the same application (catalog servers use this as the segment ID — see the [catalog object model](../query-and-transform/catalog-object-model.md)).
- **`StoreInfo`** wraps the `StoreId` and adds:
    - **`cloned_from`** — for stores that originated as a clone of another (typically the active blueprint is derived from a default blueprint).
    - **`store_source`** — where the store came from (`PythonSdk`, `RustSdk`, `CppSdk`, or a file source such as CLI / drag-drop).
    - **`store_version`** — the Rerun version that produced the data.

Matching `application_id` and `recording_id` is how the Viewer merges multiple `.rrd` files (or multiple stores within one file) into a single logical recording.

`.rbl` is just an RRD whose store happens to have `kind = Blueprint` — nothing in the bytes makes it special.
The convention of using `.rbl` for blueprints instead of `.rrd` is purely a filename hint to the Viewer and users.

When an RRD holds multiple [stores](#stores) each store begins with its own `SetStoreInfo`, and every subsequent `ArrowMsg` is tagged with its store's `StoreId`.
Messages from different stores may be interleaved or grouped.
The [footer](#footer) indexes each store separately, so readers can enumerate stores and select the ones they want without scanning chunk bytes.


## Footer

The footer is an optional manifest appended at the end of an RRD that enables random access into the file.
For each chunk in the RRD, the manifest carries chunk-level metadata (id, byte offset in the file, byte size — compressed and uncompressed) along with per-component and per-timeline statistics and the chunk's schema hash.
Like all data in Rerun, the manifest is internally stored as an Arrow `RecordBatch`, with one row per chunk.

With the footer, a reader can enumerate stores in a handful of seeks and pull only the chunks it actually needs — for example, by entity path or by time range — without reading any chunk it does not care about.
This is what enables [`RrdReader`](chunk-processing-api.md) to be cheap to use on large files, and the OSS catalog server to "load" large datasets quickly and with little memory overhead.

All tooling included in recent versions of the Rerun SDK emit footers by default.
An RRD may still miss a footer for a variety of reasons — for example, when a stream is not shut down cleanly, or legacy RRDs written before footers existed.
In those cases, readers fall back to a linear scan, which is semantically equivalent — just slower for partial reads.

For illustration, let's see what a footer looks like in an RRD.
This can be done with the following command:

```sh
rerun rrd print --footers --footers-lod 2 my.rrd
```

Here we use `--footers-lod 2` to see the entire table, which happen to be very wide. Here is the result for the recording produced by the snippet above:

```text
Showing data after migration to latest Rerun version
StoreInfo {
    store_id: StoreId(
        Recording,
        "rerun_example_rrd_format",
        "example",
    ),
    cloned_from: None,
    store_source: PythonSdk(
        3.11.13,
    ),
    store_version: Some(
        CrateVersion {
            major: 0,
            minor: 33,
            patch: 0,
            meta: Some(
                DevAlpha {
                    alpha: 1,
                    commit: None,
                },
            ),
        },
    ),
}
StoreInfo {
    store_id: StoreId(
        Recording,
        "rerun_example_rrd_format",
        "example",
    ),
    cloned_from: None,
    store_source: PythonSdk(
        3.11.13,
    ),
    store_version: Some(
        CrateVersion {
            major: 0,
            minor: 33,
            patch: 0,
            meta: Some(
                DevAlpha {
                    alpha: 1,
                    commit: None,
                },
            ),
        },
    ),
}
Chunk(chunk_18B0AA9F967A41276952b7c6bb9f6ed2) with 1 rows (632 B) - /__properties - data columns: [RecordingInfo:start_time]
Chunk(chunk_18B0AA9FA7B7B1A61d23c55ca87b18b4) with 2 rows (1.2 KiB) - /points - data columns: [Points3D:colors Points3D:positions]
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     │
│ * source: "/tmp/rrd_format_doc.rrd"                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           │
│ * schema_sha_256: 03bea0095483cf5d32a3d28fc28f0433d917cdeacf88a07fcf02b97a778f492b                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌─────────────────────┬────────────────────────────────────┬────────────────────────┬───────────────────────┬───────────────────────┬───────────────────────┬──────────────────────────────┬─────────────────────────────────┬────────────────────────────────────┬──────────────────────────────────────────┬──────────────┬──────────────┬─────────────────┬─────────────────┬────────────────────────────┬────────────────────────────┬─────────────────────────────┬────────────────────────────┬────────────────────────────────┬────────────────────────────────┬───────────────────────────────┬───────────────────────────────────┬────────────────────────────────┬──────────────────────────────┬───────────────────────────────────┬───────────────────────────────────┬─────────────────────────────────┬──────────────────────────────────────┬────────────────────────────────┬──────────────────────────────┬───────────────────────────────────┬───────────────────────────────────┬─────────────────────────────────┬──────────────────────────────────────┐ │
│ │ chunk_entity_path   ┆ chunk_id                           ┆ chunk_is_static        ┆ chunk_num_rows        ┆ chunk_byte_offset     ┆ chunk_byte_size       ┆ chunk_byte_size_uncompressed ┆ Points3D:colors:has_static_data ┆ Points3D:positions:has_static_data ┆ RecordingInfo:start_time:has_static_data ┆ frame:start  ┆ frame:end    ┆ log_tick:start  ┆ log_tick:end    ┆ log_time:start             ┆ log_time:end               ┆ frame:Points3D:colors:start ┆ frame:Points3D:colors:end  ┆ frame:Points3D:colors:num_rows ┆ frame:Points3D:positions:start ┆ frame:Points3D:positions:end  ┆ frame:Points3D:positions:num_rows ┆ log_tick:Points3D:colors:start ┆ log_tick:Points3D:colors:end ┆ log_tick:Points3D:colors:num_rows ┆ log_tick:Points3D:positions:start ┆ log_tick:Points3D:positions:end ┆ log_tick:Points3D:positions:num_rows ┆ log_time:Points3D:colors:start ┆ log_time:Points3D:colors:end ┆ log_time:Points3D:colors:num_rows ┆ log_time:Points3D:positions:start ┆ log_time:Points3D:positions:end ┆ log_time:Points3D:positions:num_rows │ │
│ │ ---                 ┆ ---                                ┆ ---                    ┆ ---                   ┆ ---                   ┆ ---                   ┆ ---                          ┆ ---                             ┆ ---                                ┆ ---                                      ┆ ---          ┆ ---          ┆ ---             ┆ ---             ┆ ---                        ┆ ---                        ┆ ---                         ┆ ---                        ┆ ---                            ┆ ---                            ┆ ---                           ┆ ---                               ┆ ---                            ┆ ---                          ┆ ---                               ┆ ---                               ┆ ---                             ┆ ---                                  ┆ ---                            ┆ ---                          ┆ ---                               ┆ ---                               ┆ ---                             ┆ ---                                  │ │
│ │ type: non-null Utf8 ┆ type: non-null FixedSizeBinary(16) ┆ type: non-null Boolean ┆ type: non-null UInt64 ┆ type: non-null UInt64 ┆ type: non-null UInt64 ┆ type: non-null UInt64        ┆ type: non-null Boolean          ┆ type: non-null Boolean             ┆ type: non-null Boolean                   ┆ type: Int64  ┆ type: Int64  ┆ type: Int64     ┆ type: Int64     ┆ type: Timestamp(ns)        ┆ type: Timestamp(ns)        ┆ type: Int64                 ┆ type: Int64                ┆ type: UInt64                   ┆ type: Int64                    ┆ type: Int64                   ┆ type: UInt64                      ┆ type: Int64                    ┆ type: Int64                  ┆ type: UInt64                      ┆ type: Int64                       ┆ type: Int64                     ┆ type: UInt64                         ┆ type: Timestamp(ns)            ┆ type: Timestamp(ns)          ┆ type: UInt64                      ┆ type: Timestamp(ns)               ┆ type: Timestamp(ns)             ┆ type: UInt64                         │ │
│ │                     ┆                                    ┆                        ┆                       ┆                       ┆                       ┆                              ┆ archetype: Points3D             ┆ archetype: Points3D                ┆ archetype: RecordingInfo                 ┆ index: frame ┆ index: frame ┆ index: log_tick ┆ index: log_tick ┆ index: log_time            ┆ index: log_time            ┆ archetype: Points3D         ┆ archetype: Points3D        ┆ archetype: Points3D            ┆ archetype: Points3D            ┆ archetype: Points3D           ┆ archetype: Points3D               ┆ archetype: Points3D            ┆ archetype: Points3D          ┆ archetype: Points3D               ┆ archetype: Points3D               ┆ archetype: Points3D             ┆ archetype: Points3D                  ┆ archetype: Points3D            ┆ archetype: Points3D          ┆ archetype: Points3D               ┆ archetype: Points3D               ┆ archetype: Points3D             ┆ archetype: Points3D                  │ │
│ │                     ┆                                    ┆                        ┆                       ┆                       ┆                       ┆                              ┆ component: Points3D:colors      ┆ component: Points3D:positions      ┆ component: RecordingInfo:start_time      ┆              ┆              ┆                 ┆                 ┆                            ┆                            ┆ component: Points3D:colors  ┆ component: Points3D:colors ┆ component: Points3D:colors     ┆ component: Points3D:positions  ┆ component: Points3D:positions ┆ component: Points3D:positions     ┆ component: Points3D:colors     ┆ component: Points3D:colors   ┆ component: Points3D:colors        ┆ component: Points3D:positions     ┆ component: Points3D:positions   ┆ component: Points3D:positions        ┆ component: Points3D:colors     ┆ component: Points3D:colors   ┆ component: Points3D:colors        ┆ component: Points3D:positions     ┆ component: Points3D:positions   ┆ component: Points3D:positions        │ │
│ │                     ┆                                    ┆                        ┆                       ┆                       ┆                       ┆                              ┆ component_type: Color           ┆ component_type: Position3D         ┆ component_type: Timestamp                ┆              ┆              ┆                 ┆                 ┆                            ┆                            ┆ component_type: Color       ┆ component_type: Color      ┆ component_type: Color          ┆ component_type: Position3D     ┆ component_type: Position3D    ┆ component_type: Position3D        ┆ component_type: Color          ┆ component_type: Color        ┆ component_type: Color             ┆ component_type: Position3D        ┆ component_type: Position3D      ┆ component_type: Position3D           ┆ component_type: Color          ┆ component_type: Color        ┆ component_type: Color             ┆ component_type: Position3D        ┆ component_type: Position3D      ┆ component_type: Position3D           │ │
│ │                     ┆                                    ┆                        ┆                       ┆                       ┆                       ┆                              ┆ index: rerun:static             ┆ index: rerun:static                ┆ index: rerun:static                      ┆              ┆              ┆                 ┆                 ┆                            ┆                            ┆ index: frame                ┆ index: frame               ┆ index: frame                   ┆ index: frame                   ┆ index: frame                  ┆ index: frame                      ┆ index: log_tick                ┆ index: log_tick              ┆ index: log_tick                   ┆ index: log_tick                   ┆ index: log_tick                 ┆ index: log_tick                      ┆ index: log_time                ┆ index: log_time              ┆ index: log_time                   ┆ index: log_time                   ┆ index: log_time                 ┆ index: log_time                      │ │
│ ╞═════════════════════╪════════════════════════════════════╪════════════════════════╪═══════════════════════╪═══════════════════════╪═══════════════════════╪══════════════════════════════╪═════════════════════════════════╪════════════════════════════════════╪══════════════════════════════════════════╪══════════════╪══════════════╪═════════════════╪═════════════════╪════════════════════════════╪════════════════════════════╪═════════════════════════════╪════════════════════════════╪════════════════════════════════╪════════════════════════════════╪═══════════════════════════════╪═══════════════════════════════════╪════════════════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════╪══════════════════════════════════════╪════════════════════════════════╪══════════════════════════════╪═══════════════════════════════════╪═══════════════════════════════════╪═════════════════════════════════╪══════════════════════════════════════╡ │
│ │ /__properties       ┆ 18b0aa9f967a41276952b7c6bb9f6ed2   ┆ true                   ┆ 1                     ┆ 240                   ┆ 986                   ┆ 1736                         ┆ false                           ┆ false                              ┆ true                                     ┆ null         ┆ null         ┆ null            ┆ null            ┆ null                       ┆ null                       ┆ null                        ┆ null                       ┆ 0                              ┆ null                           ┆ null                          ┆ 0                                 ┆ null                           ┆ null                         ┆ 0                                 ┆ null                              ┆ null                            ┆ 0                                    ┆ null                           ┆ null                         ┆ 0                                 ┆ null                              ┆ null                            ┆ 0                                    │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ /points             ┆ 18b0aa9fa7b7b1a61d23c55ca87b18b4   ┆ false                  ┆ 2                     ┆ 1242                  ┆ 1564                  ┆ 3656                         ┆ false                           ┆ false                              ┆ false                                    ┆ 0            ┆ 1            ┆ 0               ┆ 1               ┆ 2026-05-18T13:04:15.500740 ┆ 2026-05-18T13:04:15.501658 ┆ 0                           ┆ 1                          ┆ 2                              ┆ 0                              ┆ 1                             ┆ 2                                 ┆ 0                              ┆ 1                            ┆ 2                                 ┆ 0                                 ┆ 1                               ┆ 2                                    ┆ 2026-05-18T13:04:15.500740     ┆ 2026-05-18T13:04:15.501658   ┆ 2                                 ┆ 2026-05-18T13:04:15.500740        ┆ 2026-05-18T13:04:15.501658      ┆ 2                                    │ │
│ └─────────────────────┴────────────────────────────────────┴────────────────────────┴───────────────────────┴───────────────────────┴───────────────────────┴──────────────────────────────┴─────────────────────────────────┴────────────────────────────────────┴──────────────────────────────────────────┴──────────────┴──────────────┴─────────────────┴─────────────────┴────────────────────────────┴────────────────────────────┴─────────────────────────────┴────────────────────────────┴────────────────────────────────┴────────────────────────────────┴───────────────────────────────┴───────────────────────────────────┴────────────────────────────────┴──────────────────────────────┴───────────────────────────────────┴───────────────────────────────────┴─────────────────────────────────┴──────────────────────────────────────┴────────────────────────────────┴──────────────────────────────┴───────────────────────────────────┴───────────────────────────────────┴─────────────────────────────────┴──────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The first lines decode the `SetStoreInfo` messages and the `ArrowMsg` payloads found in the stream.
The wide table that follows is the manifest itself, with one row per chunk.
The chunk-level columns (entity path, id, sortedness, row count, byte span, uncompressed size) are followed by per-component and per-timeline statistics — global timeline ranges (`frame:start`/`end`, `log_tick:*`, `log_time:*`) and per-component-per-timeline statistics (`frame:Points3D:positions:start`/`end`/`num_rows`, …) — that let an indexed reader skip components within a timeline range without reading their payloads.

## File layout

This section gives a byte-level walkthrough of the framing. All multibyte integers are little-endian.

The high-level shape of any RRD is:

```
┌─────────────────────────────────────────────────────────┐
│ StreamHeader                                  12 bytes  │
├─────────────────────────────────────────────────────────┤
│ Message₁  :  MessageHeader (16 B) + payload (N₁ B)      │
│ Message₂  :  MessageHeader (16 B) + payload (N₂ B)      │
│ …                                                       │
├─────────────────────────────────────────────────────────┤
│ End msg   :  MessageHeader (16 B) + RrdFooter payload   │  ┐
│                                                         │  │ optional
│ StreamFooter                          32 bytes (typ.)   │  │ footer
└─────────────────────────────────────────────────────────┘  ┘
```

The three building blocks are detailed below.

### Stream header

Every RRD opens with the same fixed 12 bytes:

```
            StreamHeader — 12 bytes
┌────────────┬────────────┬─────────────────────┐
│   FourCC   │  Version   │   EncodingOptions   │
│  4 bytes   │  4 bytes   │       4 bytes       │
└────────────┴────────────┴─────────────────────┘
0            4            8                    12
```

| offset     | field       | value                                                       |
|------------|-------------|-------------------------------------------------------------|
| `[0..4)`   | FourCC      | `b"RRF2"` for the current format                            |
| `[4..8)`   | Version     | 4-byte encoded Rerun crate version                          |
| `[8]`      | compression | `0` = Off, `1` = LZ4                                        |
| `[9]`      | serializer  | `2` = Protobuf (`1` once meant MsgPack and is now rejected) |
| `[10..12)` | reserved    | `0x00 0x00`                                                 |

- Older `"RRF0"` / `"RRF1"` FourCCs are recognized but rejected with `OldRrdVersion` — there is no in-place reader for them, you have to migrate through an older Rerun release.
- The historical bit-pattern `[0, 0, 0, 0]` for `Version` is interpreted as `0.2.0` (pre-2023-02-27 files); any encoded version older than `0.23` is rejected outright.
- `EncodingOptions` describes how the *payloads* of subsequent messages are encoded. In practice these flags are mostly advisory today — the values that matter ride alongside each individual message — but the bytes are still part of the format and the two reserved bytes must be zero.

> [!NOTE]
> The header format exposes legacy details that are no longer supported and may require an older Rerun SDK version to migrate.
> However, RRDs created by Rerun SDK 0.23 and later are guaranteed to be migrated, and this guarantee holds for future releases — see the next section.

### Message framing

After the header, the file is a sequence of framed messages. Each message is a 16-byte header followed by an opaque payload:

```
                MessageHeader — 16 bytes
┌───────────────────────────┬───────────────────────────┐
│           kind            │       payload_len         │
│          u64 LE           │          u64 LE           │
└───────────────────────────┴───────────────────────────┘
0                           8                          16

╔════════════════════════════════╗
║   payload — payload_len bytes  ║
║   protobuf                     ║
╚════════════════════════════════╝
```

| offset    | field         | value                                                |
|-----------|---------------|------------------------------------------------------|
| `[0..8)`  | `kind`        | `MessageKind` discriminant (see table below)         |
| `[8..16)` | `payload_len` | byte length of the protobuf payload that follows     |

The `kind` field tells the decoder how to interpret the payload:

| Value | `MessageKind`                | Payload                                                                               |
|------:|------------------------------|---------------------------------------------------------------------------------------|
|   `0` | `End`                        | An `RrdFooter` protobuf (the optional footer — see below)                             |
|   `1` | `SetStoreInfo`               | A `SetStoreInfo` protobuf (announces a store)                                         |
|   `2` | `ArrowMsg`                   | An `ArrowMsg` protobuf wrapping a chunk's Arrow IPC bytes (optionally LZ4-compressed) |
|   `3` | `BlueprintActivationCommand` | A `BlueprintActivationCommand` protobuf                                               |

The outer payload bytes are always plain protobuf. Of the four kinds, only `ArrowMsg` can carry compressed data: its `compression` field tracks whether the wrapped Arrow IPC bytes are LZ4-compressed, so different `ArrowMsg`s in the same file can mix compressed and uncompressed Arrow IPC payloads.
The `compression` byte in the `StreamHeader`'s `EncodingOptions` is advisory only — per-message decoding does not consult it.

### Stream footer

The footer is written in two parts.

The first part is a regular framed message: an `End`-kind `MessageHeader` followed by the `RrdFooter` protobuf payload described in the [Footer](#footer) section.
It lives somewhere in the message stream — usually right before the file is closed — and is no different from any other framed message structurally.

The second part is the **`StreamFooter` trailer** at EOF. It is *not* a framed message: it is a raw structure whose job is to let readers jump straight to the `RrdFooter`(s) from the end of the file.
The trailer is a variable-length entry table — one 20-byte `StreamFooterEntry` per `RrdFooter` in the stream — followed by a fixed 12-byte tail that always sits at the very end of the file:

```
StreamFooter = num_entries × StreamFooterEntry + 12-byte static tail

┌──────────────────────────────────────────────┬─────────────────────────┐
│ entries[0..num_entries) — 20·num_entries B   │   static tail — 12 B    │
└──────────────────────────────────────────────┴─────────────────────────┘
EOF − 12 − 20·num_entries                      EOF − 12                EOF
```

Each `StreamFooterEntry` is 20 bytes:

| offset     | field   | value                                                                          |
|------------|---------|--------------------------------------------------------------------------------|
| `[0..8)`   | `start` | u64 LE — byte offset of the `RrdFooter` payload (after its own MessageHeader)  |
| `[8..16)`  | `len`   | u64 LE — length of the `RrdFooter` payload                                     |
| `[16..20)` | `crc32` | u32 LE — `xxh32(payload)` with the fixed seed `7850921` (`"RERUN"` in base-26) |

The static tail is the part with a known offset from EOF:

| offset (from EOF)   | field         | value                              |
|---------------------|---------------|------------------------------------|
| `[-12..-8)`         | FourCC        | `b"RRF2"`                          |
| `[-8..-4)`          | identifier    | `b"FOOT"`                          |
| `[-4..0)`           | `num_entries` | u32 LE — number of entries above   |

The CRC only covers the `RrdFooter` payload, not the surrounding `MessageHeader`, so it can be checked independently of message framing.

Reading the footer therefore boils down to:

```
1. seek EOF − 12                     → read FourCC, identifier, num_entries
2. seek EOF − 12 − 20·num_entries    → read num_entries entries
3. for each entry:
       seek entry.start, read entry.len bytes
       check xxh32(bytes, seed=7850921) == entry.crc32
       decode protobuf RrdFooter
```

A file can legally have more than one trailer — that happens when streams are simply concatenated (`cat a.rrd b.rrd > both.rrd`), but tools like `rerun rrd merge` collapse them back into a single trailer with a single entry.
Files written without a footer (streaming sinks, legacy producers) skip the `End` message and the trailer entirely; readers detect their absence by the missing `FOOT` identifier and fall back to a linear scan.


## Stability

The format is split into two layers with different stability stories.

### Binary format

This concerns the binary structure of the RRD file.
The framing described in [File layout](#file-layout) is considered **stable** and we have no plans to change it.
Legacy RRDs whose `Version` field is older than `0.23` are currently rejected.
That's the cut-off below which we do not attempt to migrate at load time; a manual hop through an older Rerun SDK release is required.
The same is true for RRDs whose FourCC is `RRF0` or `RRF1`.

Should we need to break framing compatibility again, the FourCC will bump (`RRF3`, …) and load-time auto-migration will be provided.
The `rerun rrd migrate` CLI will also be available for offline batch conversion.

### Sorbet

We refer to the high-level data model specification as Sorbet.
Its reference implementation lives in the Rust `re_sorbet` crate.
This includes the chunk and footer schemas, as well as the high-level data model (timelines, archetypes, components, etc. — see [Entities and Components](entity-component.md)).

Sorbet is versioned and **subject to change**, but `re_sorbet` performs in-memory migration to the current Sorbet version as chunks (and the footer manifest) are loaded.
Any CLI tool that rewrites an RRD (`rerun rrd merge`, `rerun rrd optimize`, `rerun rrd migrate`, …) emits chunks in the current Sorbet version, so a round-trip through any of these is also a migration.
Future changes to Sorbet will be auto-migrated in the same way.

