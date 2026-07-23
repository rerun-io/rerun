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

Each `Field.path` is a column name from the dataset's catalog schema. The decoder turns that column into a tensor:

- `NumericDecoder()` for scalar and list-of-scalar columns
- `ImageDecoder()` for encoded image blobs (JPEG/PNG)
- `VideoFrameDecoder(codec=…)` for compressed video (`h264`/`h265`/`av1`)

The dict keys (`"state"`, `"action"`, …) in `fields` become the keys of each sample dict that the dataset yields. When the `index` is a timestamp timeline (like `"real_time"` above), pass `timeline_sampling=FixedRateSampling(rate_hz=…)` so the dataloader knows how to lay out the sampling grid.

#### Action chunks and history via `window`

`Field(window=(start, end))` returns a *slice* of values across that inclusive range relative to the current index, instead of a single value:

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/vla-with-history.jpg" alt="Sample with non-uniform history showing the current row plus a windowed slice of preceding rows">
</picture>

snippet: howto/dataloader[window]

The example uses this to feed 50-step action chunks into the ACT policy.

#### Video decoding is GOP-aware

A `VideoFrameDecoder` looks like a regular field from the outside, but decoding any one frame of compressed video requires running the codec from the previous keyframe forward through the target frame. The chain of frames the codec has to walk through is bounded by the [GOP](https://en.wikipedia.org/wiki/Group_of_pictures) length:

<picture>
  <img src="https://rerun.io/blog/data-layer-tax/sample-construction-with-gops.jpg" alt="Sample construction for a VLA model: each video frame requires decoding from the preceding keyframe forward">
</picture>

snippet: howto/dataloader[video_decoder]

The dataloader handles this transparently. `VideoFrameDecoder.context_range` asks the prefetcher for a window of preceding samples ending at the target, sized to be guaranteed to span at least one keyframe; the codec runs over the fetched packets in order and returns the frame at the target index. You only need to pass `keyframe_interval`, which must be greater than or equal to the actual GOP length; for timestamp timelines, also pass an `fps_estimate` that approximates the true frame rate.

### Iterable vs. Map-style

The dataloader provides both PyTorch dataset styles:

- `RerunIterableDataset`: streaming iteration with internal shuffling (on by default) and cross-worker partitioning. Good default. Call `ds.set_epoch(epoch)` to reseed the shuffle between epochs.
- `RerunMapDataset`: random access by global index, plugs into PyTorch's sampler ecosystem (`DistributedSampler`, `WeightedRandomSampler`, `SubsetRandomSampler`, …).

Wrap either in `torch.utils.data.DataLoader`:

snippet: howto/dataloader[dataloader]

For DDP, the iterable dataset partitions the index list across ranks automatically. With the map dataset, swap in `sampler=DistributedSampler(ds)` and call `sampler.set_epoch(epoch)` each epoch.

### Shuffling and fetch locality

`RerunIterableDataset` takes a `shuffle_strategy` argument that controls the order samples are *fetched* in:

- `SampleShuffle()` (the default): every sample lands at an independent random position.
  Batches are maximally decorrelated, but every fetch scatters across all segments, so the server re-reads shared storage (e.g. video [GOPs](https://en.wikipedia.org/wiki/Group_of_pictures)) on every fetch.
- `BlockShuffle(block_size=…)`: cuts the sample space into blocks of consecutive samples (never crossing a segment boundary) and shuffles the block order plus the sample order within each block.
  Each fetch then reads one contiguous span, so the server reads each storage chunk about once per epoch instead of once per fetch.
  `block_size` defaults to the dataset's `fetch_size`.
- `NoShuffle()`: natural order, maximal fetch locality, no randomness.

For video-heavy datasets, `BlockShuffle` can speed up epochs by an order of magnitude, because decoding one frame requires fetching its whole GOP: with scattered fetches the same GOP chunks are re-fetched over and over.

The trade-off of `BlockShuffle` is that consecutive samples now come from the same contiguous block, so batches are correlated.
The `shuffle_buffer_size` argument is the second half of that strategy: decoded samples pass through a shuffle buffer of that size and leave it in random order, mixing samples from many blocks (and thus many segments) into each batch without changing which data is fetched when.
Randomization improves smoothly with buffer size: the residual chance that two samples in a batch come from the same block falls off roughly as `block_size / buffer_size`, so every doubling of the buffer halves the correlation.
A buffer of a few times `block_size × batch_size` already gets batches close to what a full per-sample shuffle would produce, and returns diminish from there.
There is no downside beyond resources, though: if you have the RAM, a bigger buffer only mixes better.
The costs are holding that many decoded samples in memory per DataLoader worker, and startup latency: emission starts once the buffer is half full and the buffer keeps filling while training runs, so a large buffer delays the first batch by half its size rather than all of it.

The shuffle buffer only makes sense together with `BlockShuffle`.
With `SampleShuffle` the fetch order is already fully random, so re-shuffling it on emission adds memory cost without any benefit.
With `NoShuffle` the buffer only jumbles nearby samples, which typically means reordering within a single segment, nowhere near the cross-segment mixing training needs.

```python
ds = RerunIterableDataset(
    source=source,
    index="frame_index",
    fields=fields,
    shuffle_strategy=BlockShuffle(),
    shuffle_buffer_size=4096,
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
