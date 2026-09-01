---
title: "0.37"
order: 973
---

## Highlights

Rerun 0.37 teaches the Viewer about **assets**: static data registered on a dataset and shared by all of its segments, such as the robot mesh every episode refers to.
Open a segment and the Viewer downloads its dataset's assets once, then reuses them for every other segment, and you can register and unregister assets straight from the Viewer.
The selection panel got a full overhaul with a readable heading, chevrons for walking the entity tree, and a selection history, while state timeline views now keep the time cursor centered, accept entities dropped from the streams panel, and have a **Visible time range** setting.

The experimental PyTorch dataloader was rebuilt around a single fetch-and-decode pipeline shared by the iterable and map-style datasets, with explicit windows that work across every decoder, opt-in controls that cut the cost of decoding overlapping video windows, and manifests that can now back a `RerunMapDataset`.
On the ingestion side, `rerun rrd stats` tells you whether a recording is worth optimizing, `VideoStream:is_keyframe` always gets a chunk of its own so a reader can scan a video's keyframes without downloading any samples, and Rust users can now pick their file importers a la carte.

Finally, `rerun.datatypes` is now `rerun.encodings` across all three SDKs, with the old spelling deprecated but still working.

## New features

### Assets

[Assets](../concepts/query-and-transform/catalog-object-model.md#assets) are static data registered on a dataset and shared by its segments, such as a robot mesh that every episode refers to.

When you open a segment that has an asset the viewer downloads the asset, and caches it so other segments in the dataset don't
have to download the same asset again.

A dataset in the viewer now also list assets and certain metadata about them:

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/ca548403d20f8e39c4756f73851514956aff7345_assets_short.mp4" type="video/mp4" />
</video>

As seen in the video, assets can also be registered, and unregistered in the viewer.

From Python, a segment store covers the dataset's assets alongside the segment's own data.
Pass `include_assets=False` to leave them out, which also skips the requests for their manifests:

```python
dataset.register_asset("file:///path/to/file.rrd")

# Describes the chunks of both the segment and the asset.
store = dataset.segment_store(segment_id)
for chunk in store.stream().to_chunks():
    print(chunk.entity_path)

# Describes only the segment, no asset manifests are fetched.
segment_only = dataset.segment_store(segment_id, include_assets=False)
```

### Improved selection panel navigation

The selection panel got an overhaul, the heading is now a lot more readable, you can click the chevrons to navigate
around the tree of entities, and there's a selection history now!

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/15c5c79a90337cda6601aeda088719a648e280a8_Screen Recording 2026-08-27 at 11.19.45-1787823076970.mp4" type="video/mp4" /> <!-- NOLINT -->
</video>

### State timeline views keep the time cursor centered

Long recordings no longer start out zoomed all the way out in a state timeline view: the view shows a window around the time cursor and keeps it centered while playing, like time series views do.

[Docs here.](../reference/types/views/state_timeline_view.md)
Example: https://github.com/rerun-io/rerun/blob/latest/examples/python/state_timeline

### Drag entities onto state timeline views

Entities that log a [`StateChange`](../reference/types/archetypes/state_change.md) can now be dragged from the streams panel onto a state timeline view.

[Docs here.](../reference/types/views/state_timeline_view.md)

### Visible time range for state timeline views

State timeline views now have a **Visible time range** setting in the selection panel, the same one time series views have.

It is also available from the SDK as `StateTimelineView(time_ranges=…)`.

[Docs here.](../reference/types/views/state_timeline_view.md)

### Export recordings and blueprints from the web Viewer

The [Web Viewer JavaScript API](../reference/npm.md) now provides `save_recording()` and `save_blueprint()`.
Each method returns a byte stream that can be saved as an `.rrd` or `.rbl` file.
The stream is also compatible with `open_channel()`, so applications can restore an exported artifact through normal RRD ingestion.

```ts
const artifact = new Uint8Array(
  await new Response(viewer.save_recording()).arrayBuffer(),
);
// …
const channel = viewer.open_channel();
channel.send_rrd(artifact);
```

### Links to datasets

A URI to a dataset is now `rerun://<origin>/dataset/<dataset_id>` instead of `rerun://<origin>/entry/<dataset_id>`. The older version
of the link still resolve correctly.

### `rerun rrd stats` tells you whether a recording should be optimized

`rerun rrd stats` now ends with a `Chunk index analysis` section, computed per store from the chunk index alone:

```
Chunk index analysis
--------------------

Store StoreId(Recording, "droid", "WEIRD_5047dd9a_2024_01_21_23h_22m_28s")
chunk index columns: 66

Optimization check based on a 2.0 MiB chunk size target (`--profile object-store`)
  - theoretical lower bound:   1 149 chunks
  - effective:               584 409 chunks (508.6×)
  - excess:                  583 260 chunks

⚠️ This recording may be unoptimized — consider running `rerun rrd optimize`
```

The check compares the recording's temporal chunk count against a lower bound of what merging could achieve under the object-store optimization profile.
When both the relative and absolute excess cross a threshold, the output warns about possibly unoptimized data.

The analysis requires a chunk index; run `rerun rrd migrate` first on files written before those existed.

[Docs here.](../howto/logging-and-ingestion/optimize-chunks.md)

### Keyframe markers are kept out of the video chunks

`VideoStream:is_keyframe` now always ends up in a chunk of its own, so a reader can scan a video's keyframes without downloading a single video sample.
It used to ride along in the sample chunks unless GoP rebatching happened to move it.

This is the first use of a new type-definition attribute, `#[rerun(own_chunk)]`, which marks a component as one that never shares a chunk with another component.

The split runs wherever chunks are optimized, and it is not optional — unlike the thick/thin split, no setting turns it off:

* `rerun rrd optimize`
* `rerun.experimental.OptimizationProfile`, and the Python chunk-processing pipeline that takes one
* `ChunkStore::compacted` and `ChunkStore::finalize_compaction` in Rust

The chunk store also refuses to merge such a chunk back together with anything else, so a recording keeps the layout once it has it.

### A la carte features for file importers

`re_importer` now gates every import format behind its own Cargo feature — `image`, `lerobot`, `mcap`, `parquet`, `urdf`, and `video` — and the `rerun` crate gates native video handling behind a new `video` feature.
All of them are on by default, so nothing changes unless you opt out.

Rust users who only need a few formats can now skip the rest and cut compile times.
For example, a project that only imports URDF:

```toml
re_importer = { version = "0.37", default-features = false, features = ["urdf"] }
```

Dropping the other importers cut roughly 20% off both debug and release build times in the author's project.

`re_sdk`'s `importers` feature still enables the full set, so `RecordingStream::log_file_from_path` keeps working with every format.
To benefit from the split, depend on `re_importer` directly and pick your formats there.

### Experimental dataloaders use a unified fetch and decode pipeline

The experimental PyTorch dataloaders now use the same explicit stages to plan queries, fetch Arrow data, resolve decoder requests, and batch-decode samples.
This makes behavior consistent across iterable and map-style datasets as well as live-catalog and manifest-backed loading.

OpenTelemetry tracing for `RerunIterableDataset` now covers both fetch paths and distinguishes block fetching, batch decoding, exposed fetch latency, and downstream pull delays.
Decode spans end before samples are yielded, so training-loop behavior no longer inflates decode durations.

[Docs here.](../howto/train/dataloader.md)

### Faster video decoding for training

The experimental PyTorch dataloader now provides three opt-in controls that reduce the cost of decoding overlapping video windows:

- `window_storage="view"` stores each unique decoded frame once per contiguous run and returns views into the shared frame bank.
- `output_format="yuv420p"` skips CPU RGB conversion and keeps compact YUV planes through collation, ready for transfer and conversion on the GPU.
- `thread_count` exposes FFmpeg frame threading for H.264 and H.265 streams.

Because video views share storage, callers must not mutate window values in place before collation.
The existing single-threaded, copied RGB output remains the default.

```python
video_decoder = VideoFrameDecoder(
    codec="h264",
    thread_count=4,
    window_storage="view",
    output_format="yuv420p",
)

loader = DataLoader(dataset, collate_fn=Yuv420Collator(), pin_memory=True)

for batch in loader:
    batch["video"] = batch["video"].to_rgb("cuda", non_blocking=True)
```

[Docs here.](../howto/train/dataloader.md)

### `RerunMapDataset` can be built from a manifest

`RerunMapDataset.from_manifest(manifest, source, fields)` builds a map-style dataset over the validated samples of a frozen `Manifest`, mirroring `RerunIterableDataset.from_manifest`.

This is a performance optimization: the manifest already holds the validated sample set and each field's frozen decode range, so the dataset skips the live scan at construction and the per-batch keyframe lookup when fetching.

```python
manifest = Manifest.from_parquet("epoch.parquet")
dataset = RerunMapDataset.from_manifest(manifest, source, fields)
loader = DataLoader(dataset, batch_size=8, sampler=DistributedSampler(dataset))
```

The manifest's recorded order is **not** replayed here.
Ordering and cross-worker sharding stay with the `DataLoader`'s sampler, as for any map-style dataset, so this cannot reproduce a manifest's run, use `RerunIterableDataset.from_manifest` for reproducible, resumable training.

[Docs here.](../howto/train/dataloader.md)

### Faster manifest generation for video datasets

Manifest generation for the experimental PyTorch dataloader now uses sparse `VideoStream:is_keyframe` timestamps to validate compressed-video fields and anchor their decode ranges.
It no longer scans `VideoStream:sample` timestamps, avoiding the transfer of expensive encoded video data while building a manifest.
For compressed video, `max_staleness` is conservatively measured from the latest prior keyframe, so a sample may be omitted even when a fresher non-keyframe exists.

[Docs here.](../howto/train/dataloader.md)

### Experimental iterable dataloader skips missing samples

The live `RerunIterableDataset` now skips samples when a decoder returns `None`, warning once per missing field.
`NumericDecoder` returns `None` for a window whose rows have inconsistent widths, allowing the affected live sample to be skipped instead of stopping iteration.
Null source rows and expected encoded-image or video decode failures also resolve to `None`, with video failures isolated to the affected GOP.
Filtering happens before the optional emission shuffle so incomplete samples do not occupy its buffer.
The `max_consecutive_skipped_samples` option defaults to 100 and caps the number skipped in a row by each rank and `DataLoader` worker before iteration raises with total and per-field counts; pass `None` to disable the limit.
Because missing samples are skipped after rank sharding, finite DDP training loops must use `DistributedDataParallel.join()` so one rank finishing early does not stall the others.

Manifest replay remains strict: if a field recorded as required no longer resolves, iteration raises an error asking the user to regenerate the manifest rather than silently changing its frozen sample order.
Valid zero-sized tensors are preserved; only `None` denotes missing data.

[Docs here.](../howto/train/dataloader.md)

## Breaking changes

### `datatypes` renamed to `encodings`

`datatype` meant two different things in Rerun: the low-level types that components are built from, and the Arrow `DataType` those are stored as.
The first of the two is now called an **encoding**, so `rerun.datatypes` is `rerun.encodings`.

The old spelling still works, deprecated, and will be removed in a future release.

Python:

```py
rr.datatypes.Vec3D([1, 2, 3])  # before
rr.encodings.Vec3D([1, 2, 3])  # after
```

Rust:

```rust
use re_types::datatypes::Vec3D;  // before
use re_types::encodings::Vec3D;  // after
```

C++ — the namespace alias keeps working, but the per-type include paths moved:

```cpp
#include <rerun/datatypes/vec3d.hpp>  // before
#include <rerun/encodings/vec3d.hpp>  // after
```

Reference pages moved from `reference/types/datatypes/…` to `reference/types/encodings/…`; the old URLs redirect.

[Docs here.](../reference/types/encodings.md)

### Update to Rust 1.96

The Rust SDK now requires Rust 1.96 or later.
Run `rustup update` to get it.

### Setting a recording id no longer disables recording properties

Properties are now sent regardless of setting the recording id for Rust and C++, making it consistent with Python.

#### Rust

```rust
// Now sends properties:
let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_app")
    .recording_id("run-1")
    .recording_started(epoch)
    .save("run-1.rrd")?;
```

To get the old behavior back, opt out explicitly:

```rust
// To get the old behavior:
let rec = rerun::RecordingStreamBuilder::new("rerun_example_my_app")
    .recording_id("run-1")
    .send_properties(false) // new
    .save("run-1.rrd")?;
```

C++ also has a new `send_properties` opt-out as a constructor parameter:

```cpp
// Sends properties:
const auto rec = rerun::RecordingStream("rerun_example_my_app", "run-1");

// Does not, same as old behavior:
const auto rec = rerun::RecordingStream("rerun_example_my_app", "run-1", rerun::StoreKind::Recording, false);
```

[Docs here.](../concepts/query-and-transform/properties-and-segments.md)

### `rerun-sdk[datafusion]` and `rerun-sdk[dataplatform]` extras removed

Both extras were deprecated in 0.33 in favor of `rerun-sdk[catalog]`, and are now gone.
`pip install` fails on an unknown extra, so update any `pyproject.toml`, `requirements.txt`, `uv` dependency group, or install script that still names them.

| Before                                  | After                            |
|-----------------------------------------|----------------------------------|
| `pip install rerun-sdk[datafusion]`     | `pip install rerun-sdk[catalog]` |
| `pip install rerun-sdk[dataplatform]`   | `pip install rerun-sdk[catalog]` |

The dependency set is unchanged — `catalog` installs the same `datafusion` and `pandas` versions the old extras did.

[Docs here.](../getting-started/install-rerun/python.md)

### `Loggable` replaced by four (de)serialization traits

**Rust SDK only.** The Python and C++ SDKs are unaffected, and so is the data format — the Arrow
encodings are byte-for-byte identical.

This only matters if you implement your own components or encodings in Rust, which is rare.
Logging built-in archetypes and components needs no changes.

The `Loggable` trait bundled five functions, forcing every type to provide all of them even when some were meaningless.
Types whose Arrow encoding is never nullable had to supply a `to_arrow_opt` that failed at runtime, and a type that implemented only serialization got default `from_arrow`/`from_arrow_opt` bodies that call each other, recursing forever.

`Loggable` is gone. The functions now live in separate traits, so a type implements only what makes sense for it:

`Loggable::arrow_datatype` is also renamed to `ArrowDataType::arrow_data_type`, to match how Arrow itself spells it.

| Trait           | Function(s)                        | Status                       |
| --------------- | ---------------------------------- | ---------------------------- |
| `ArrowDataType` | `arrow_data_type`, `arrow_empty`   | supertrait of the other four |
| `ToArrow`       | `to_arrow`                         | required by `Component`      |
| `ToArrowOpt`    | `to_arrow_opt`                     | optional                     |
| `FromArrow`     | `from_arrow`, `verify_arrow_array` | required by `Component`      |
| `FromArrowOpt`  | `from_arrow_opt`                   | optional                     |

`Component` now requires `ToArrow + FromArrow`: a component must round-trip, but does not have to be nullable.

The nullable variants are only implemented where they are actually needed, so most built-in types no longer have them.
Of the roughly 90 built-in encodings, 19 do — the ones that appear as a nullable field of another type, such as `Utf8`, `Blob`, `ImageFormat`, `PixelFormat` and `TensorBuffer`.
Components inherit the traits from the encoding they wrap, so 11 of them have the nullable variants (`Text`, `Name`, `MediaType`, `ImageBuffer`, …) and the rest, including `Position2D` and `Color`, do not.
Prefer the non-nullable variants in new code.

#### Migration

Split your `impl Loggable` into one `impl` per trait, and import the specific traits you call.
Implement `ToArrow` and `FromArrow`; only add the `*Opt` variants if your type is used as a nullable field of another type.

Before:

```rust
use rerun::Loggable as _;

impl rerun::Loggable for Confidence {
    fn arrow_datatype() -> arrow::datatypes::DataType {
        rerun::Float32::arrow_datatype()
    }

    fn to_arrow_opt<'a>(
        data: impl IntoIterator<Item = Option<impl Into<std::borrow::Cow<'a, Self>>>>,
    ) -> rerun::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        rerun::Float32::to_arrow_opt(data.into_iter().map(|opt| opt.map(Into::into).map(|c| c.0)))
    }
}
```

After:

```rust
impl rerun::ArrowDataType for Confidence {
    fn arrow_data_type() -> arrow::datatypes::DataType {
        <rerun::Float32 as rerun::ArrowDataType>::arrow_data_type()
    }
}

impl rerun::ToArrow for Confidence {
    fn to_arrow<'a>(
        data: impl IntoIterator<Item = impl Into<std::borrow::Cow<'a, Self>>>,
    ) -> rerun::SerializationResult<arrow::array::ArrayRef>
    where
        Self: 'a,
    {
        <rerun::Float32 as rerun::ToArrow>::to_arrow(data.into_iter().map(Into::into).map(|c| c.0))
    }
}

impl rerun::FromArrow for Confidence {
    fn from_arrow(
        data: &dyn arrow::array::Array,
    ) -> rerun::DeserializationResult<Vec<Self>> {
        Ok(<rerun::Float32 as rerun::FromArrow>::from_arrow(data)?
            .into_iter()
            .map(Confidence)
            .collect())
    }
}
```

A type that is never nullable is now done at that point — `RowId`, `ChunkId`, `Tuid` and `EntityPath` all skip the `*Opt` traits entirely.

If you do need the nullable variants, implement whichever direction is natural and derive the other with a macro, since sibling traits cannot supply each other's default bodies:

```rust
rerun::macros::impl_to_arrow_via_to_arrow_opt!(Confidence);      // `ToArrow` from `ToArrowOpt`
rerun::macros::impl_from_arrow_via_from_arrow_opt!(Confidence);  // `FromArrow` from `FromArrowOpt`
rerun::macros::impl_from_arrow_opt_via_from_arrow!(Confidence);  // `FromArrowOpt` from `FromArrow`
```

Call sites that did `use rerun::Loggable as _;` should import the traits whose functions they actually call, e.g. `use rerun::{FromArrow as _, ToArrow as _};`.

#### Batches of optional components

`Vec<Option<C>>`, `[Option<C>; N]` and `[Option<C>]` implement `ComponentBatch` only when `C: ToArrowOpt`.
Since most components no longer implement it, logging a batch with gaps in it no longer compiles for them:

```rust
// No longer compiles: `Position2D` is not `ToArrowOpt`.
vec![Some(Position2D::new(1.0, 2.0)), None].serialized(descriptor)

// Still fine: `Text` wraps `Utf8`, which keeps the nullable traits.
vec![Some(Text::from("a")), None].serialized(descriptor)
```

Log the present values instead, or use `RecordingStream::send_columns` with explicit partition lengths if you need to express absence.

Example: [`custom_data`](https://github.com/rerun-io/rerun/blob/main/docs/snippets/all/tutorials/custom_data.rs)

### `Mp4Reader` emits `is_keyframe` as a sparse marker chunk

Stream mode used to put `VideoStream:is_keyframe` on every sample chunk, one `true`/`false` row per sample.
It now emits a single trailing chunk holding only `is_keyframe`, with one `true` row per keyframe:

```
# 0.35
[0] static -> VideoStream:codec
[1] GOP 0 -> VideoStream:is_keyframe, VideoStream:sample
[2] GOP 1 -> VideoStream:is_keyframe, VideoStream:sample

# 0.36
[0] static -> VideoStream:codec
[1] GOP 0 -> VideoStream:sample
[2] GOP 1 -> VideoStream:sample
[3] marker -> VideoStream:is_keyframe (only `true` rows)
```

GOP rebatching rejects `is_keyframe=false` rows, so the old dense column made `collect(optimize=…)` skip video rebatching entirely unless you passed `fix_keyframe=True`.
It now works with no extra flag, and the marker is kept verbatim instead of rebuilt.
A keyframe-only query (such as the dataloader's frame anchor) can also skip the sample column now that the two no longer share a chunk.

Code that post-processes the reader's stream has to tell the marker chunk apart from the sample chunks.
It is temporal but holds no samples, so logic keyed on `not chunk.is_static` will treat it as samples.
Key on the `VideoStream:sample` column instead:

```python
is_sample_chunk = "VideoStream:sample" in chunk.to_record_batch().schema.names
```

This bites hardest in a `map` that rewrites the time column (e.g. retagging mp4 PTS onto a wall-clock timeline): a positional cursor advances over the marker chunk and stamps it with the wrong time.

### Experimental dataloader decoders now process fetch blocks

The experimental PyTorch dataloader now calls each field decoder once per fetch block, passing a [`FieldBatch`][rerun.experimental.dataloader.FieldBatch] and a sequence of [`DecodeRequest`][rerun.experimental.dataloader.DecodeRequest] objects.
This lets numeric decoders gather samples in a vectorized operation and video decoders share work across requests from the same GOP.

Custom [`ColumnDecoder`][rerun.experimental.dataloader.ColumnDecoder] implementations must update their `decode` method from the per-sample API:

```python
def decode(self, raw, index_value, segment_id):
    return tensor
```

to the batch API, returning one result per request in the same order:

```python
def decode(self, batch, requests):
    return [tensor]
```

The `fetch_size` argument and manifest metadata field have also been renamed to `fetch_block_size` to clarify that fetching and decoding use the same block.

[Docs here.](../howto/train/dataloader.md)

### Experimental dataloader Windows are explicit and decoder-independent

`Field.window` now specifies the exact offsets to return relative to each sample instead of an inclusive `(start, end)` range.
Windows work with every built-in decoder, including numeric values, Arrow data, images, and compressed video.
Video windows fetch the intermediate GOP frames required for decoding while returning only the explicitly requested frames.

Integer timelines use integral index-step offsets.
Timestamp and duration timelines use seconds, which are converted to nanoseconds internally; `max_staleness` now follows the same convention.

For example, migrate an integer window as follows:

```python
# Before: every index from -2 through 0.
Field(path, decode=decoder, window=(-2, 0))

# After: the exact same three index offsets.
Field(path, decode=decoder, window=(-2, -1, 0))
```

For a timestamp timeline sampled at 10 Hz, migrate nanosecond values to explicit seconds:

```python
# Before: the inclusive 500 ms history range, with staleness in nanoseconds.
Field(path, decode=decoder, window=(-500_000_000, 0), max_staleness=500_000_000)

# After: six explicit samples, with both values expressed in seconds.
Field(
    path,
    decode=decoder,
    window=tuple(step / 10.0 for step in range(-5, 1)),
    max_staleness=0.5,
)
```

Compressed-video fields now require their sibling `VideoStream:is_keyframe` component so decode ranges can reliably begin at a keyframe.
Custom decoders should use `DecodeRequest.decode_row_indices` and `DecodeRequest.output_row_indices`; the decoder-owned `context_range` hook has been removed because fetch requirements are now resolved by the field pipeline.

[Docs here.](../howto/train/dataloader.md)

---

Looking for an older release? See the [migration guides for 0.33 and earlier](../reference/migration.md).
