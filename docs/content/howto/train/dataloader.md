---
title: Train PyTorch models with Rerun
order: 200
description: Stream Rerun recordings into a PyTorch DataLoader for model training, without an intermediate export step.
---

Train PyTorch models directly from a Rerun server.

The experimental [`dataloader`](https://github.com/rerun-io/rerun/tree/main/rerun_py/rerun_sdk/rerun/experimental/dataloader) module exposes Rerun recordings as iterable or map-style PyTorch datasets, decoding compressed video (`h264`/`h265`/`av1`), images, and scalars on the fly. Random access, multi-worker prefetching, and [DDP](https://docs.pytorch.org/tutorials/beginner/ddp_series_theory.html) partitioning all work out of the box.

> [!WARNING]
> **Experimental.** The API is provisional and will change between releases. For large-scale training, [Rerun Hub](https://rerun.io) offers a higher-performance backend than the OSS catalog.

The full code for this guide lives in [`examples/python/dataloader/`](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader), which trains a [LeRobot ACT](https://tonyzhaozh.github.io/aloha/) policy from a HuggingFace dataset.

<picture>
  <img src="https://static.rerun.io/howto-dataloader/c635e994f9d1591811816821173813c54fe440ef/full.png" alt="">
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

The dataloader assembles those samples on demand from the per-recording [chunks](https://rerun.io/docs/concepts/logging-and-ingestion/chunks) in a Rerun [catalog](https://rerun.io/docs/concepts/query-and-transform/catalog-object-model#catalog), while the PyTorch `DataLoader` drives batching, shuffling, and worker parallelism.

## How to use it

### Register data with a catalog

The dataloader reads from a Rerun catalog, so you must first register [RRDs](https://rerun.io/docs/concepts/logging-and-ingestion/recordings/#storage-formats). Start the OSS server in a separate terminal:

```bash
rerun server
```

Then register your recordings. Each registered RRD becomes a *segment* in the dataset, typically one episode or trajectory per RRD:

snippet: howto/dataloader[register]

The example's [`prepare_dataset.py`](https://github.com/rerun-io/rerun/blob/main/examples/python/dataloader/prepare_dataset.py) shows the full flow for converting a HuggingFace LeRobot dataset into per-episode RRDs and registering them.

### Describe a sample

A Rerun dataset is built from three things:

- a `DataSource`: the catalog dataset and an optional segment filter
- an `index`: the timeline that defines what "one sample" means (e.g. `"real_time"` or `"frame_index"`)
- a dict of `Field`s: what each sample should contain

snippet: howto/dataloader[describe_sample]

Each `Field.path` is a column name from the dataset's catalog schema.
The decoder turns that column into a training value:

- `NumericDecoder()` for scalar and list-of-scalar columns
- `ImageDecoder()` for encoded image blobs (JPEG/PNG)
- `VideoFrameDecoder(codec=…)` for compressed video (`h264`/`h265`/`av1`)

The dict keys (`"state"`, `"action"`, …) in `fields` become the keys of each sample dict that the dataset yields. When the `index` is a timestamp timeline (like `"real_time"` above), pass `timeline_sampling=FixedRateSampling(rate_hz=…)` so the dataloader knows how to lay out the sampling grid.

#### Action chunks and history via `window`

`Field(window=(offset, …))` returns one value per explicit offset relative to the current index, instead of a single value.
Integer timelines use integral index-step offsets. Timestamp and duration timelines use seconds, which the dataloader converts to nanoseconds internally; callers never need to match the Arrow timestamp storage unit.
`Field(max_staleness=…)` uses the same convention when limiting the age of the nearest value during manifest generation.

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/vla-with-history.jpg" alt="Sample with non-uniform history showing the current row plus a windowed slice of preceding rows">
</picture>

snippet: howto/dataloader[window]

The example uses this to feed 50-step action chunks into the ACT policy.
With `VideoFrameDecoder`, a window returns one decoded frame per explicit offset as a `[T, 3, H, W]` tensor.
For GPU training, `VideoFrameDecoder(output_format="yuv420p", window_storage="view")` instead returns compact YUV planes, shares decoded storage across overlapping windows, and defers RGB conversion until after device transfer.
With `NumericDecoder`, every resolved numeric-list row in a window must have the same width, producing a `[T, D]` tensor (including `[T, 1]` for a scalar component).
Variable-width window rows decode to `None` rather than being flattened and losing their time boundaries; the live iterable dataset treats that sample as missing.

Unwindowed variable-sized values such as point clouds remain supported as one tensor per sample.
Batch them with a custom `collate_fn` that pads and returns a mask, samples a fixed number of elements, or concatenates values and returns batch offsets.
Padding belongs in the collator because only it knows the maximum size of the actual training batch.

#### Video decoding is GOP-aware

A `VideoFrameDecoder` looks like a regular field from the outside, but decoding any one frame of compressed video requires running the codec from the previous keyframe forward through the target frame. The chain of frames the codec has to walk through is bounded by the [GOP](https://en.wikipedia.org/wiki/Group_of_pictures) length:

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/sample-construction-with-gops.jpg" alt="Sample construction for a VLA model: each video frame requires decoding from the preceding keyframe forward">
</picture>

snippet: howto/dataloader[video_decoder]

The dataloader handles this transparently by looking up the prior keyframe, fetching the encoded samples from that keyframe through the requested outputs, and decoding them in order.
The video stream must include the sibling `VideoStream:is_keyframe` component, either logged with the data or generated by an optimized collection pass.

To avoid CPU RGB conversion and reduce the size of decoded samples, keep video in YUV420 through collation.
`Yuv420Collator` is an optional convenience for applications whose other fields can use PyTorch's default collation:

```python
from torch.utils.data import DataLoader

from rerun.experimental.dataloader import Yuv420Collator

loader = DataLoader(dataset, collate_fn=Yuv420Collator(), pin_memory=True)

for batch in loader:
    batch["video"] = batch["video"].to_rgb("cuda", non_blocking=True)
```

Applications can instead compose YUV handling into their own collator with `Yuv420Frame.stack`:

```python
from rerun.experimental.dataloader import Yuv420Frame


def collate(samples):
    return {
        "video": Yuv420Frame.stack([sample["video"] for sample in samples]),
        "state": custom_state_collation(samples),
    }


loader = DataLoader(dataset, collate_fn=collate, pin_memory=True)

for batch in loader:
    batch["video"] = batch["video"].to_rgb("cuda", non_blocking=True)
```

Call `to_rgb` in the training process rather than inside a `DataLoader` worker when converting on CUDA.

### Iterable vs. Map-style

The dataloader provides both PyTorch dataset styles:

- `RerunIterableDataset`: streaming iteration with internal shuffling (on by default) and cross-worker partitioning. Good default. Call `ds.set_epoch(epoch)` to reseed the shuffle between epochs.
- `RerunMapDataset`: random access by global index, plugs into PyTorch's sampler ecosystem (`DistributedSampler`, `WeightedRandomSampler`, `SubsetRandomSampler`, …).

Wrap either in `torch.utils.data.DataLoader`:

snippet: howto/dataloader[dataloader]

For DDP, the iterable dataset partitions the index list across ranks automatically. With the map dataset, swap in `sampler=DistributedSampler(ds)` and call `sampler.set_epoch(epoch)` each epoch.

> [!WARNING]
> When a finite live `RerunIterableDataset` is consumed to exhaustion under DDP, wrap the training loop in [`DistributedDataParallel.join()`](https://docs.pytorch.org/docs/stable/generated/torch.nn.parallel.DistributedDataParallel.html#torch.nn.parallel.DistributedDataParallel.join):
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

The live `RerunIterableDataset` drops a sample when any decoder returns `None` and warns once for each missing field.
`None` is the decoder contract for unresolved data; a valid zero-sized tensor is still yielded.
Null source rows and expected encoded-image or video decode failures are converted to `None`; a corrupt video GOP does not prevent later GOPs in the fetch block from decoding.
Custom decoders should return `None` for recoverable data errors; exceptions still propagate as decoder failures.
Set `max_consecutive_skipped_samples` to bound how many such samples each rank and `DataLoader` worker may skip in a row; a valid sample resets the count, and exceeding the limit raises with total and per-field counts.
The default is 100; pass `None` to apply no limit.

Live samples are partitioned across workers and DDP ranks before decoding, so uneven missing data can make their yielded sample counts differ.
Use the DDP `join()` pattern above when consuming the live dataset to exhaustion.
For deterministic multi-rank training, generate a `Manifest` with the appropriate `required_fields`: invalid samples are removed before sharding, and replay raises if a required field has disappeared or no longer decodes instead of silently changing the frozen epoch.

`RerunMapDataset` cannot skip a missing item without changing its stable index mapping, so its sample dictionary may contain `None` for an unresolved field.

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

```python
ds = RerunIterableDataset(
    source=source,
    index="frame_index",
    fields=fields,
    shuffle_strategy=BlockShuffle(buffer_size=4096),
)
```

### Train

From there, the training loop is standard PyTorch:

snippet: howto/dataloader[train]

The full [LeRobot ACT example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader) wires this up against three camera streams plus state and action chunks, and trains the policy end-to-end.

## Limitations

The module is **experimental**: expect breaking changes between releases as we iterate on the design.

For large-scale training (hundreds of recordings, multi-node), consider [Rerun Hub](https://rerun.io), which offers a higher-performance backend than the OSS catalog.

## References

- [LeRobot ACT training example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader)
- [`rerun.experimental.dataloader`](https://github.com/rerun-io/rerun/tree/main/rerun_py/rerun_sdk/rerun/experimental/dataloader) module source
- [The data layer tax in robot learning](https://rerun.io/blog/data-layer-tax) (figures used in this guide)
- [Export recordings to LeRobot datasets](lerobot_export.md) (inverse: Rerun → LeRobot dataset)
