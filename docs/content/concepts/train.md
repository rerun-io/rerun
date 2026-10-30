---
title: Train
order: 400
description: Feed catalog recordings into training pipelines via export or PyTorch
---

A Rerun [catalog](query-and-transform/catalog-object-model.md) can feed training pipelines two ways: export recordings to a standard format, or stream them directly into a PyTorch `DataLoader`.

## Export to a training format

The catalog exposes recordings as queryable DataFrames via [DataFusion](https://datafusion.apache.org/python/).
Multi-rate sensor streams can be time-aligned and columns of interest extracted, with the result written to whatever format a training pipeline expects.

See [Export recordings to LeRobot datasets](../howto/train/lerobot_export.md) for a worked example.

## Train directly from the catalog

The experimental [`rerun.experimental.dataloader`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/) module wraps a catalog as iterable or map-style PyTorch datasets, with no intermediate export step.

### Sample space

Three things describe a dataset (see [reference](https://ref.rerun.io/docs/python/stable/experimental_dataloader/)):

- **[`DataSource`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.DataSource)** — a catalog `DatasetEntry` with an optional segment filter; each registered RRD is one *segment*, typically one episode or trajectory
- **`index`** — the timeline that defines what "one sample" means (e.g. `"frame_index"` or `"real_time"`)
- **`fields`** — a dict of [`Field`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.Field)s, each mapping a source column (an `entity:Archetype:component` triple) to a decoder

[`SampleIndex`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.SampleIndex) computes the sample space from per-segment index-range metadata.
For integer timelines, leave `timeline_sampling` unset.
For timestamp and duration timelines, `timeline_sampling=FixedRateSampling(rate_hz=…)` defines the sampling grid; fields that use `fill_latest_at` resolve each grid point to the latest recorded value.
The sampling choice must match the timeline type.
Shuffling controls sample order separately through `shuffle_strategy`.

For fields already aligned on a dense, zero-based integer frame grid, `timeline_sampling=TimeAlignedSampling(keyframe_interval=10)` enables exact-index reads across all selected fields and calculates video decode starts from keyframes at `0, 10, 20, …`.
The keyframe interval is required, and the caller must validate the [data requirements](../howto/train/dataloader.md#video-decoding-is-gop-aware).
The default `timeline_sampling=None` preserves each field's filling behavior.

### Decoders

Each `Field` has a [`ColumnDecoder`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.ColumnDecoder) that converts a raw Arrow column to a training value:

- [`NumericDecoder`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.NumericDecoder) — scalars and numeric lists
- [`ImageDecoder`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.ImageDecoder) — JPEG/PNG blobs
- [`VideoFrameDecoder`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/#rerun.experimental.dataloader.VideoFrameDecoder) — compressed video (`h264`/`h265`/`av1`) as RGB tensors or compact YUV420 planes

### Windows

`Field(window=(offset, …))` returns one value per explicit relative offset rather than a single value.
Offsets must use the unit of the dataset's selected index timeline: integral index units for integer timelines, and seconds for timestamp or duration timelines.
This is how action chunks and observation history are expressed.

### Dataset styles

- `RerunIterableDataset` — streaming with automatic shuffling and cross-worker and DDP partitioning
- `RerunMapDataset` — random access by global index; works with PyTorch samplers like `DistributedSampler` and `WeightedRandomSampler`

See [Train PyTorch models with Rerun](../howto/train/dataloader.md) for usage.
