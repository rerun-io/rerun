---
name: rerun-chunk-processing
description: "Core mechanics of the Rerun Chunk Processing API (rerun.experimental) — LazyChunkStream pipelines, Chunk, lenses (MutateLens/DeriveLens/Selector), RrdReader, writing optimized RRDs. Read BEFORE writing any ingestion/conversion/preprocessing code (convert an MCAP, build a recording from a dataset, preprocess an .rrd, port an old converter): it mandates reader+lens pipelines and steers away from hand-built chunks — no Chunk.from_columns for data a reader/lens can produce, no per-message rr.log, no manual pa.array assembly. Source-specific knowledge lives in the importer skills (rerun-mcap, rerun-urdf, rerun-parquet, rerun-lerobot); read rerun-data-model first to decide what the data should become."
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun chunk processing

The pipeline layer between raw data and an RRD: readers produce `Chunk`s,
streams transform them, terminal calls execute. This skill is the generic
mechanics only. Decide the data model first (`rerun-data-model`), then pick the
importer skill for each source:

| Source                                    | Reader                                  | Skill           |
| ----------------------------------------- | --------------------------------------- | --------------- |
| MCAP file (ROS2, protobuf, Foxglove)      | `McapReader(path).stream()`             | `rerun-mcap`    |
| URDF robot model (+ joint states → FK)    | `UrdfTree.from_file_path(...).stream()` | `rerun-urdf`    |
| Parquet table (trajectories, sensor logs) | `ParquetReader(path).stream()`          | `rerun-parquet` |
| LeRobot dataset directory                 | built-in importer, then `RrdReader`     | `rerun-lerobot` |
| Existing RRD                              | `RrdReader(path)`                       | here, below     |
| Sidecar files (JSON calib, metadata)      | `Chunk.from_columns` + `from_iter`      | here, below     |

The API is `rerun.experimental`; when
behavior matters, check the installed surface:
`python -c "from rerun.experimental import LazyChunkStream; help(LazyChunkStream)"`.

## Decision rule: where does each component come from?

Default: **a reader produces the chunks; lenses shape them.** Walk this before
writing any conversion code — most "build it by hand" instincts are wrong here:

1. **Source a reader supports?** Use the reader's `.stream()`; never hand-parse
   and re-log. MCAP→`McapReader`, URDF→`UrdfTree`, parquet→`ParquetReader`,
   RRD→`RrdReader`, LeRobot dir→`log_file_from_path`.
2. **A decoder already emits the archetype?** Foxglove gives `Transform3D`,
   `Pinhole`, `VideoStream` (real sample bytes) ready-made — **pass it through**,
   do not re-derive. Only custom-protobuf topics arrive as `<Name>:message` and
   need a lens (see `rerun-mcap`).
3. **Fix an existing component in place** (swapped resolution, recolor, unit
   convert)? `MutateLens`, `output_mode="forward_unmatched"`.
4. **Derive a new component/entity** (FK→`/tf`, scalars from a message)?
   `DeriveLens`. To scatter one row into N (a joint batch → per-joint `/tf`),
   use the **two-lens pair**: derive the batch with `output_mode="forward_all"`
   (keeps the originals, e.g. the joint states), then a second
   `DeriveLens` with `scatter=True` and `output_mode="drop_unmatched"` (emits
   only the scattered rows). See the `robot_data_preprocessing` example.
5. **Genuine sidecar** no reader or lens can produce (JSON calibration offsets,
   hand-measured extrinsics, external metadata)? `Chunk.from_columns` + `from_iter`.
6. Finish with `LazyChunkStream.merge(...)` →
   `.collect(optimize=OptimizationProfile.OBJECT_STORE)` →
   `write_rrd(application_id, recording_id)`.

Why this order: the pipeline stays lazy, columnar, multithreaded, and
`OBJECT_STORE`-optimizable. A hand-built row loop or an out-of-lens `pa.array`
throws all of that away — that is the path we are deliberately avoiding.

## Anti-patterns (use a reader + lens instead)

If you are writing the left, stop and use the right:

- **`for`-loop building rows/components** → a lens with a `Selector(...).pipe(...)`
  PyArrow-compute callback.
- **`rr.init` + `rr.log` per message for conversion** → that is _live_ logging;
  for ingestion, read with a reader and `write_rrd`.
- **`chunk.to_record_batch()` + `pc.filter` then rebuilding via
  `Chunk.from_columns`** (row-thinning by hand) → `stream.drop(content=...)`,
  `.split(...)`, or a `MutateLens` returning a filtered `pa.array`.
- **`pa.array` / `pa.RecordBatch` / `np.frombuffer` assembled OUTSIDE a lens** →
  move the transform inside a `MutateLens`/`DeriveLens` selector callback.
- **`rr.send_columns` hand-assembled from a custom parser** → use the matching
  reader; it produces chunks directly.
- **Parsing MCAP/URDF with a non-Rerun library then re-logging** → `McapReader`
  / `UrdfTree`.
- **`Chunk.from_columns` for data a reader already decodes** (`Pinhole`
  intrinsics, `VideoStream`, `Transform3D` from a transforms topic) → keep it in
  the reader stream; fix with a `MutateLens` if needed.

A wall of `pyarrow.compute` "missing-attribute" type errors (`pc.filter`,
`pc.list_element`) usually means `pc.*` calls sit in module-level helpers instead
of inside `Selector.pipe` lens callbacks. Refactor into a lens before suppressing
the checker — the errors are a smell that the hand-building should not exist.

**Porting an existing converter?** Hand-built converters predate decoder
improvements and are not ground truth. Re-verify the decoder output (step 2) and
check every `Chunk.from_columns` / for-loop against this list before copying.

## Core model

- `LazyChunkStream` is a lazy pipeline DAG, not a collection. Building
  filters, lenses, maps, splits, and merges reads no source data.
- Execution starts at terminal calls: `write_rrd(...)`, `collect()`,
  `to_chunks()`, or iterating the stream.
- Execution is streaming, multithreaded, and mostly GIL-free. Prefer
  stream/lens operations and PyArrow compute over Python row loops.
- **Move semantics**: builder calls (`filter`, `drop`, `lenses`, `map`,
  `flat_map`) consume the input stream; reusing a consumed stream raises.
  Reassign after each step. Terminal calls do not consume, but each terminal
  call re-executes the whole pipeline; `collect()` once if that is too costly.
- `ChunkStore` is materialized in memory (`stream.collect()`,
  `ChunkStore.from_chunks`). `LazyStore` is manifest-indexed, loads chunks on
  demand (`RrdReader(path).store()`, catalog segment stores). Both have
  `schema()`, `summary()`, `stream()`, and `write_rrd(...)`.

## Stream composition

```python
from rerun.experimental import Chunk, LazyChunkStream, OptimizationProfile
```

- `stream.filter(content=, has_timeline=, is_static=, components=)` keeps the
  matching portion of each chunk; `stream.drop(...)` is its complement, same
  keyword filters. `content` takes an entity-path glob or a list of them.
- `stream.map(fn)` applies `Chunk -> Chunk`; `stream.flat_map(fn)` applies
  `Chunk -> Iterable[Chunk]`. Escape hatches for chunk-level Python logic;
  prefer lenses for columnar work.
- `stream.split(content=, ...)` returns `(matching, non_matching)`; both
  branches share the same upstream.
- `LazyChunkStream.merge(*streams)` fans in any number of sources.
- `LazyChunkStream.from_iter(chunks)` wraps hand-built chunks.

```python
stream = source_stream()  # any importer skill
stream = stream.drop(content="/video_raw/**")
stream = stream.lenses(fix_lens, content="/cam/**", output_mode="forward_unmatched")
merged = LazyChunkStream.merge(stream, sidecar_stream)
merged.write_rrd(out_path, application_id="my_app", recording_id=recording_id)
```

## Hand-built chunks — sidecar only

Use `Chunk.from_columns` ONLY for data no reader or lens can emit — JSON/CSV
calibration, frame offsets, external metadata. If a reader
(`McapReader`/`UrdfTree`/`ParquetReader`) decodes the topic or a lens can derive
it, that is the idiomatic path; do not hand-assemble it here. In the
`robot_data_preprocessing` example the _only_ hand-built chunk is the JSON
offsets sidecar; the camera fix, FK→`/tf`, meshes, and recolor are all
readers + lenses.

`Chunk.from_columns(entity_path, indexes, columns)` mirrors
`rr.send_columns(...)` and accepts the same archetype `.columns(...)` helpers.
Empty `indexes` means static.

```python
chunk = Chunk.from_columns(
    "/tf_static/robot_offsets",
    indexes=[],  # static
    columns=rr.Transform3D.columns(
        translation=translations,
        quaternion=quaternions_xyzw,
        parent_frame=parents,
        child_frame=children,
    ),
)
sidecar_stream = LazyChunkStream.from_iter([chunk])
```

`rr.AnyValues.columns(...)` covers non-standard metadata fields. For
inspection, a `Chunk` exposes `entity_path`, `num_rows`, `is_static`,
`timeline_names`, `to_record_batch()`, and `format()` (human-readable table).

## Lenses

Lenses reshape, fix, or derive components without iterating rows. Apply with
`stream.lenses(lenses, output_mode=..., content=...)`.

- `MutateLens(component, selector, keep_row_ids=False)` modifies an existing
  component in place.
- `DeriveLens(component, output_entity=None, scatter=False)` creates new
  columns, optionally at another entity. Chain `.to_component(descriptor,
selector)` per output; `.to_timeline(name, "sequence" | "duration_ns" |
"timestamp_ns", selector)` extracts a time column from the data itself.
  `scatter=True` explodes one input row into N output rows (one per list
  element).
- Scope with `content=` whenever the same component name exists under multiple
  entities.

Output modes, and **the default is `drop_unmatched`**:

- `drop_unmatched` (default): only lens outputs survive. Right for derive-only
  intermediate streams; silently discards everything else if applied broadly.
- `forward_unmatched`: lens outputs plus the original components no lens
  consumed. Right for targeted fixes that preserve the rest of the stream.
- `forward_all`: lens outputs plus all originals, including consumed ones. Can
  duplicate data.

In-place fix (keep Arrow type and length intact):

```python
stream = stream.lenses(
    MutateLens(
        "Pinhole:resolution",
        Selector(".").pipe(
            lambda res: pa.array(
                [(h, w) for w, h in res.to_pylist()],
                type=res.type,
            )
        ),
    ),
    content=["/external/cam_low", "/external/cam_high"],
    output_mode="forward_unmatched",
)
```

Derive with unit conversion (PyArrow compute, no Python loop):

```python
DeriveLens("schemas.proto.JointState:message", output_entity="/joints_deg/waist").to_component(
    rr.Scalars.descriptor_scalars(),
    Selector(".joint_positions").pipe(lambda arr: pc.multiply(pc.list_element(arr, 0), 180.0 / math.pi)),
)
```

## Selector grammar

`Selector("<query>")` navigates nested Arrow data, jq-style:

- `.` current value; `.field` struct field
- `[]` iterate list elements; `[N]` index a list
- `?` suppress errors / skip missing optionals; `!` assert non-null
- `|` pipe one expression into another

`.pipe(fn)` chains a Python/PyArrow transform (or another Selector).
`.execute(array)` runs it eagerly; `.execute_per_row(array)` guarantees the
output row count matches the input (use inside lens callbacks that must stay
row-aligned).

## Writing RRDs

- `stream.write_rrd(path, application_id=..., recording_id=...)` executes and
  writes in one streaming pass.
- `stream.collect(optimize=OptimizationProfile.OBJECT_STORE).write_rrd(...)`
  materializes, optimizes chunk layout, then writes. Memory scales with the
  materialized chunks.
- Profiles: `OBJECT_STORE` (large chunks, for storage/query/catalog) and
  `LIVE` (small chunks, low-latency viewer).
- Multiple physical RRDs form one logical recording when they share a
  `recording_id`; use this to separate base data, model/URDF data, and layers.

**Always use `OptimizationProfile.OBJECT_STORE`** when the RRD is headed for a
Rerun catalog or Hub, unless explicitly asked otherwise.

## Chunk API vs logging API

- Logging (`rr.log`, `rr.send_columns`, `RecordingStream`) is for live logging
  from user code; chunk processing is for ingestion, conversion, and
  postprocessing existing recordings.
- Logging → chunks: write an RRD, read it back with `RrdReader`.
  `RrdReader(path)` lists `recordings()` / `blueprints()` (each a `StoreEntry`
  with `kind`, `application_id`, `recording_id`); `.stream(store=entry)` for
  sequential passes, `.store(store=entry)` for indexed access.
- Chunks → logging: `rerun.experimental.send_chunks(chunks, recording=...)`
  accepts a `Chunk`, `LazyChunkStream`, `LazyStore`, `ChunkStore`, or any
  iterable of chunks. The source store's `application_id`/`recording_id` are
  **not** preserved; the active recording's identity wins.

## Common gotchas

- The default lens `output_mode` is `drop_unmatched`; forgetting to set
  `forward_unmatched` on a targeted fix silently drops the rest of the stream.
- Do not reuse a consumed `LazyChunkStream`; reassign or `split` deliberately.
- Scope lenses with `content=`; the same component name often exists under
  many entities.
- Preserve Arrow array type and length in `MutateLens` transforms.
- For catalog layers, the layer `recording_id` must equal the segment id.
- This is `rerun.experimental`; pin-check signatures when upgrading.

## References

- End-to-end example (MCAP + URDF + JSON sidecar, lenses, merge, optimize):
  `https://github.com/rerun-io/rerun/tree/main/examples/python/robot_data_preprocessing`
- Docs: `https://rerun.io/docs/concepts/logging-and-ingestion/chunk-processing-api`,
  `https://rerun.io/docs/concepts/query-and-transform/lenses`
