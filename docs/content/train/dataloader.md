---
title: Train PyTorch models with Rerun
order: 200
description: Stream Rerun recordings into a PyTorch DataLoader without an export step
---

Train PyTorch models directly from a Rerun server.

> [!NOTE]
> The examples on this page use a local open-source server and also run on [Rerun Hub](../catalog/hub.md) ([how to switch](../catalog/hub.md#run-the-examples-on-hub)).

The experimental [`dataloader`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/) module exposes Rerun recordings as iterable or map-style PyTorch datasets, decoding compressed video (`h264`/`h265`/`av1`), images, and scalars on the fly. Random access, multi-worker prefetching, and [DDP](https://docs.pytorch.org/tutorials/beginner/ddp_series_theory.html) partitioning all work out of the box.

> [!NOTE]
> **Experimental.** The API is provisional and will change between releases.

The full code for this guide lives in [`examples/python/dataloader/`](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader), which trains a [LeRobot ACT](https://tonyzhaozh.github.io/aloha/) policy from a HuggingFace dataset.

<picture>
  <img src="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/full.png" alt="Rerun Viewer showing a robot arm episode with a 3D view, joint and gripper position plots, three camera feeds and a language instruction">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/1200w.png">
</picture>

## Training sample construction

A [vision-language-action policy](https://en.wikipedia.org/wiki/Vision-language-action_model) is trained on samples that align several columns of multimodal data at the same instant in time:

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/vla.jpg" alt="A single training sample for a VLA model with camera, task, state, and action columns aligned at the current row">
</picture>

The dataloader assembles those samples on demand from the per-recording [chunks](../log/chunks-and-rrd/chunks.md) in a Rerun [catalog](../catalog/catalog-object-model.md#catalog), while the PyTorch `DataLoader` drives batching, shuffling, and worker parallelism.

## How to use it

Install `rerun-sdk[dataloader]` to include the dataloader's dependencies, including PyTorch 2.11 or newer.

### Register data with a catalog

The dataloader reads from a Rerun catalog, so you must first register [RRDs](../log/data-model/recordings.md#storage-formats). Start the open-source server in a separate terminal:

```bash
rerun server
```

Then register your recordings. Each registered RRD becomes a *segment* in the dataset, typically one episode or trajectory per RRD:

snippet: howto/dataloader[register]

The example's [`prepare_dataset.py`](https://github.com/rerun-io/rerun/blob/main/examples/python/dataloader/prepare_dataset.py) shows the full flow for converting a HuggingFace LeRobot dataset into per-episode RRDs and registering them.

### Describe a sample

A Rerun dataset is built from three things:

- a `DataSource`: the catalog dataset or filtered selection, with an optional curation
- an `index`: the timeline that defines what "one sample" means (e.g. `"real_time"` or `"frame_index"`)
- a dict of `Field`s: what each sample should contain

snippet: howto/dataloader[describe_sample]

`DataSource` accepts a dataset or a `DatasetSelection` created by filtering it.
Plain loading uses either directly; a curation is optional.
For a whole dataset, a filtered view, or a curation resolved from that view:

snippet: howto/dataloader[data_source]

Resolve segment and interval selections with `Curation.from_source` before saving a training manifest.
Content filters restrict visible fields without narrowing segment-wide ranges and remain loader configuration, alongside `fields`.
An empty curation produces an empty loader.
The loader rejects a curation whose dataset, timeline, or timeline type does not match its source and `index`.

Each `Field.path` is a column name from the dataset's catalog schema.
The decoder turns that column into a training value:

- `NumericDecoder()` for scalar and list-of-scalar columns
- `ImageDecoder()` for encoded image blobs (JPEG/PNG)
- `VideoFrameDecoder(codec=…)` for compressed video (`h264`/`h265`/`av1`)

The dict keys (`"state"`, `"action"`, …) in `fields` become the keys of each sample dict that the dataset yields.
For timestamp or duration timelines, pass `timeline_sampling=FixedRateSampling(rate_hz=…)` to define the sampling grid.
Each grid point takes the latest value recorded at or before it.
For integer timelines, omit `timeline_sampling`.
The sampling choice must match the timeline type; `shuffle_strategy` controls the order of the resulting samples.
Dataset construction checks the timeline and requested columns against the source's content-filtered schema without reading segment data.

#### Action chunks and history via `window`

`Field(window=(offset, …))` returns one value per explicit offset relative to the current index, instead of a single value.
Integer timelines use integral index-step offsets. Timestamp and duration timelines use seconds.

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/vla-with-history.jpg" alt="Sample with non-uniform history showing the current row plus a windowed slice of preceding rows">
</picture>

snippet: howto/dataloader[window]

The example uses this to feed 50-step action chunks into the ACT policy.
With `VideoFrameDecoder`, a window returns one decoded frame per explicit offset as a `[T, 3, H, W]` tensor.
`VideoFrameDecoder(window_storage="view")` collects decoded RGB frames in a common frame buffer and returns views for contiguous windows.
Overlapping windows share storage, so treat them as read-only until collation.
For GPU training, `VideoFrameDecoder(output_format="yuv420p", window_storage="view")` instead returns compact YUV planes, shares decoded storage across overlapping windows, and defers RGB conversion until after device transfer.
With `NumericDecoder`, every resolved numeric-list row in a window must have the same width, producing a `[T, D]` tensor (including `[T, 1]` for a scalar component).
Variable-width window rows decode to `None` rather than being flattened and losing their time boundaries; the live iterable dataset treats that sample as missing.

Unwindowed variable-sized values such as point clouds remain supported as one tensor per sample.
Batch them with a custom `collate_fn` that pads and returns a mask, samples a fixed number of elements, or concatenates values and returns batch offsets.
Padding belongs in the collator because only it knows the maximum size of the actual training batch.

For CPU tensors, NumPy arrays, numbers, and YUV frames, `rerun.experimental.dataloader.PooledBatchCollator` reuses shared CPU buffers for worker-to-consumer batches on Linux.
It supports nested dictionaries, lists, and tuples with matching structure, shapes, and data types across samples.
Worker pooling requires `multiprocessing_context="spawn"` and PyTorch's `file_descriptor` sharing strategy.
With `num_workers=0`, it batches data without pooling and also works on other platforms.
Buffers are allocated on demand and returned to the pool after the consumer releases them.
Pool limits bound cached buffers per worker; retained batches and uncached fallback allocations can use additional memory.
It works with either dataset style; see the [pooled-batch example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader_pooled_batches) for usage and ownership constraints.

Set `packed=True` to combine tensor fields into one shared buffer and use one parent-side pinning copy.
With worker processes, unpinned batches are `PackedBatch` dictionaries: a `dict` subclass containing the usual tensor fields and nested values.
When DataLoader performs accelerator pinning, it returns an ordinary dictionary with pinned tensor views.
PyTorch can disable pinning when no supported accelerator is available, even if `pin_memory=True`.
With `num_workers=0`, the collator returns an ordinary dictionary without pooling.
The training loop consumes batches directly in every case:

```python
from torch.utils.data import DataLoader

from rerun.experimental.dataloader import PooledBatchCollator

loader = DataLoader(
    dataset,
    batch_size=32,
    num_workers=4,
    multiprocessing_context="spawn",
    pin_memory=True,
    collate_fn=PooledBatchCollator(packed=True),
)

for batch in loader:
    train_step(batch)
```

Keep source tensors alive until non-blocking device transfers finish.

#### Video decoding is GOP-aware

A `VideoFrameDecoder` looks like a regular field from the outside, but decoding any one frame of compressed video requires running the codec from the previous keyframe forward through the target frame. The chain of frames the codec has to walk through is bounded by the [GOP](https://en.wikipedia.org/wiki/Group_of_pictures) length:

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/sample-construction-with-gops.jpg" alt="Sample construction for a VLA model: each video frame requires decoding from the preceding keyframe forward">
</picture>

snippet: howto/dataloader[video_decoder]

The dataloader handles this transparently by looking up the prior keyframe, fetching the encoded samples from that keyframe through the requested outputs, and decoding them in order.
The video stream must include the sibling `VideoStream:is_keyframe` component.
If your recordings do not have it, run [`rerun rrd optimize`](../reference/cli.md#rerun-rrd-optimize) on them before registering, which derives it from the stream.
Variable GOP lengths are supported.
If no preceding keyframe exists, the video field is unresolved and live iterable loading applies the same `max_consecutive_skipped_samples` policy as other missing values.

For aligned data with known video keyframes, pass `timeline_sampling=TimeAlignedSampling(keyframe_interval=10)` to the dataset.
This uses exact-index reads for all fields instead of latest-at filling and calculates video decode starts using the declared interval.
The data must satisfy:

1. **An integer timeline.** The selected timeline uses integer frame numbers.
2. **Dense, zero-based indices.** Every selected field (scalars, state, actions, video) has one value or frame at each index `0, 1, 2, …`, with no gaps or null values.
3. **Known video keyframes.** Frames `0, K, 2K, …` are valid decode starting points; for `K=10`, frame 27 decodes from frame 20.
4. **A shared frame-number reference.** The same index refers to the same physical instant across fields.
   Every selected video and segment uses the same keyframe interval and origin.
5. **Integer field-window offsets.** Windows use whole-frame steps, such as `(-2, -1, 0)`.

Validate these properties when preparing the data; the loader does not scan the dataset to check them.
It checks each computed decode start's `is_keyframe` marker alongside the requested video data as it loads.
A missing or false marker makes that video field unresolved, so live iterable loading skips the sample under the same `max_consecutive_skipped_samples` policy as other missing values.
The default `timeline_sampling=None` preserves each field's filling behavior and discovers keyframes from the data.

To validate the keyframes of video imported from MP4 with `rerun.experimental.Mp4Reader` and not transcoded, `Mp4Reader.info()` gives the keyframe frame numbers and GOP sizes from the file's metadata, without decoding it.

To avoid CPU RGB conversion and reduce the size of decoded samples, keep video in YUV420 through collation.
`Yuv420Collator` is an optional convenience for applications whose other fields can use PyTorch's default collation:

snippet: howto/dataloader_performance[yuv_collator]

Applications can instead compose YUV handling into their own collator with `Yuv420Frame.stack`:

snippet: howto/dataloader_performance[custom_collate]

Call `to_rgb` in the training process rather than inside a `DataLoader` worker when converting on CUDA.

### Train on selected spans

Dataset-level curation chooses segments; span-level curation chooses intervals within them.
Resolve either selection into a `Curation` before sampling, then save its Arrow coordinates as the training manifest.
To curate whole segments, pass a dataset selection without an annotation table:

snippet: howto/dataloader[curate_recordings]

SQL can select segments through the catalog's DataFusion context.
For example, if segments have an `episode.success` property:

snippet: howto/dataloader[curate_sql]

To combine the segment selection with annotated intervals, pass exported span coordinates or another `Curation` as `spans` to `from_source`; see [Select spans for training](../query-and-transform/dataframes/spans.md#select-spans-for-training).
The resolved intervals are the intersection of the selected segments' ranges and the selected spans.
Without `spans`, the curation contains the selected segments' full timeline ranges.

Save the resolved output, then reload it against the full same dataset without reconstructing the recording-selection view:

snippet: howto/dataloader[save_curation]

Resolving a curation reads segment metadata, without fetching or validating training fields.
Intervals select sample anchor times; field windows may read earlier or later data in the same segment.
Content filters remain loader configuration and are not stored in the manifest.
Overlapping selections do not duplicate anchors, and video decoding may also read an earlier keyframe.
For local persistence, use `curation.write_parquet("training.parquet")` and `Curation.from_parquet(dataset_entry, "training.parquet")`.
Both dataset styles accept `DataSource(dataset_or_selection, curation=curation)` through their normal constructors.
Their `from_curation()` methods remain convenience wrappers.
The sampling choice must match the curation's timeline, just as it does for live loading.

The iterable loader computes sample order and rank/worker assignments deterministically from the curation and run settings.
Reproducing them requires unchanged source data, loader version, sampling and shuffle settings, seed, epoch, and rank/worker counts.
Use `set_epoch()` to change the shuffle between epochs.
Keep `DataLoader(persistent_workers=False)` so workers receive the updated epoch when iteration restarts.

### Iterable vs. Map-style

The dataloader provides both PyTorch dataset styles:

- `RerunIterableDataset`: streaming iteration with internal shuffling (on by default) and cross-worker partitioning. Good default. Call `ds.set_epoch(epoch)` to reseed the shuffle between epochs.
- `RerunMapDataset`: random access by global index, plugs into PyTorch's sampler ecosystem (`DistributedSampler`, `WeightedRandomSampler`, `SubsetRandomSampler`, …).

Wrap either in `torch.utils.data.DataLoader`:

snippet: howto/dataloader[dataloader]

For DDP, the iterable dataset partitions the index list across ranks automatically. With the map dataset, swap in `sampler=DistributedSampler(ds)` and call `sampler.set_epoch(epoch)` each epoch.

> [!WARNING]
> When a finite `RerunIterableDataset` is consumed to exhaustion under DDP, wrap the training loop in [`DistributedDataParallel.join()`](https://docs.pytorch.org/docs/stable/generated/torch.nn.parallel.DistributedDataParallel.html#torch.nn.parallel.DistributedDataParallel.join):
>
> ```python
> with model.join():
>     for batch in dataloader:
>         loss = train_step(model, batch)
>         loss.backward()
>         optimizer.step()
>         optimizer.zero_grad()
> ```
>
> Rank shards can differ slightly in size, and missing samples are skipped after sharding, so one rank may finish before another.
> Without `join()`, the remaining rank can wait forever in the next DDP collective.
> This also applies when Ray `TorchTrainer` wraps the model in DDP: call `join()` explicitly inside `train_loop_per_worker`.
> `join()` is specific to DDP; FSDP and other distributed strategies need their own uneven-input handling or a training loop that provides the same number of steps to every rank.

### Missing values

`RerunIterableDataset` drops a sample when any decoder returns `None` and warns once for each missing field.
`None` is the decoder contract for unresolved data; a valid zero-sized tensor is still yielded.
Null source rows and expected encoded-image or video decode failures are converted to `None`; a corrupt video GOP does not prevent later GOPs in the fetch block from decoding.
Custom decoders should return `None` for recoverable data errors; exceptions still propagate as decoder failures.
Set `max_consecutive_skipped_samples` to bound how many such samples each rank and `DataLoader` worker may skip in a row; a valid sample resets the count, and exceeding the limit raises with total and per-field counts.
The default is 1000; pass `None` to apply no limit.

Samples are partitioned across workers and DDP ranks before decoding, so uneven missing data can make their yielded sample counts differ.
Skipping does not replace samples or redistribute them between workers.
Use the DDP `join()` pattern above when consuming the dataset to exhaustion.

`RerunMapDataset` cannot skip a missing item without changing its stable index mapping, so its sample dictionary may contain `None` for an unresolved field.

### Measure pipeline performance

Each Rerun dataset automatically collects cumulative fetch and decode metrics across its `DataLoader` workers.
Use the standard PyTorch `DataLoader` and read the report from the dataset when needed.

snippet: howto/dataloader_performance[metrics]

`summary()` returns a flat dictionary containing the available cumulative metrics.
It is an aggregate of completed worker measurements.
The optional prefix makes the dictionary directly usable with experiment trackers:

```python
mlflow.log_metrics(dataset.metrics.summary(prefix="dataloader."), step=epoch)
wandb_run.log(dataset.metrics.summary(prefix="dataloader/"), step=epoch)
```

The compact report answers:

- `fetch_wait_fraction` and `decode_fraction`: whether the measured worker critical path is fetch- or decode-bound.
- `fetch_requests_per_sample`: merged direct HTTP range requests plus gRPC chunk fetch requests per fetched sample target.
- `fetch_request_merge_factor`: source chunk byte ranges represented by each merged direct HTTP range request.
- `decoded_frames_per_output_frame`: video decode amplification caused by GOP traversal.
- `skipped_samples`: live iterable samples dropped because at least one decoded field was unresolved.

Fetch wait measures only latency that background lookahead did not hide.
The merge factor describes only the direct HTTP path; requests per sample combines direct and gRPC chunk fetches and is omitted when neither path issued a request.
Fetched sample targets are counted when their fetch block completes, so shuffle buffers, DataLoader prefetching, skipped outputs, and early iterator termination do not bias the denominator.
Video decode amplification counts actual codec output; contiguous block fetching should approach `1.0`, while uniformly sampled positions in a GOP of length 10 average approximately `5.5`.
Skipped samples are counted once even when multiple fields are unresolved.

### Shuffling and fetch locality

`RerunIterableDataset` takes a `shuffle_strategy` argument that controls the order samples are *fetched* in:

- `SampleShuffle()` (the default): every sample lands at an independent random position.
  Batches are maximally decorrelated, but every fetch scatters across all segments, so the server re-reads shared storage (e.g. video [GOPs](https://en.wikipedia.org/wiki/Group_of_pictures)) on every fetch.
- `BlockShuffle()`: cuts the sample space into fetch-sized blocks of consecutive samples (never crossing a segment boundary) and shuffles the block order, keeping the sample order within each block.
  Each fetch then reads one contiguous span, so the server reads each storage chunk about once per epoch instead of once per fetch.
  It is the only strategy that takes a `buffer_size` — see below.
- `NoShuffle()`: natural order, maximal fetch locality, no randomness.

For video-heavy datasets, `BlockShuffle` can speed up epochs by an order of magnitude, because decoding one frame requires fetching its whole GOP: with scattered fetches the same GOP chunks are re-fetched over and over.

The trade-off of `BlockShuffle` is that consecutive samples now come from the same contiguous block, so batches are correlated.
`BlockShuffle(buffer_size=…)` is the second half of that strategy: decoded samples pass through a shuffle buffer of that size and leave it in random order, mixing samples from many blocks (and thus many segments) into each batch without changing which data is fetched when.
Randomization improves smoothly with buffer size: the residual chance that two samples in a batch come from the same block falls off roughly as `fetch_block_size / buffer_size`, so every doubling of the buffer halves the correlation.
A buffer of a few times `fetch_block_size × batch_size` already gets batches close to what a full per-sample shuffle would produce, and returns diminish from there.

The buffer holds *decoded* samples, exactly as your decoders produce them — full-resolution RGB frames or compact YUV planes for video fields, before any resizing you do in a `collate_fn`.
Budget `buffer_size × bytes_per_sample × num_workers`, because every DataLoader worker fills its own buffer: a few thousand video samples per worker runs to tens of gigabytes.
The other cost is startup latency.
Emission starts once `min_fill` samples are buffered, which defaults to half the buffer, so a large buffer delays the first batch by half its size.
Lower `min_fill` to shorten that warm-up; the buffer keeps filling while training runs either way, so steady-state mixing is unaffected.

Only `BlockShuffle` accepts a buffer, because it is the only strategy whose fetch order stays deliberately correlated.
With `SampleShuffle` the fetch order is already fully random, so re-shuffling it on emission would cost memory without any benefit.
With `NoShuffle` the buffer would only jumble nearby samples, which typically means reordering within a single segment, nowhere near the cross-segment mixing training needs.

snippet: howto/dataloader_performance[block_shuffle]

### Train

From there, the training loop is standard PyTorch:

snippet: howto/dataloader[train]

The full [LeRobot ACT example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader) wires this up against three camera streams plus state and action chunks, and trains the policy end-to-end.

## Limitations

For large-scale training (hundreds of recordings, multi-node), consider [Rerun Hub](../catalog/hub.md), which offers a higher-performance backend than the OSS catalog. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->

## References

- [LeRobot ACT training example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader)
- [`rerun.experimental.dataloader`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/) API reference
- [The data layer tax in robot learning](https://rerun.io/blog/data-layer-tax) (figures used in this guide)
- [Export recordings to LeRobot datasets](lerobot_export.md) (inverse: Rerun → LeRobot dataset)
