---
title: Train
order: 475
---

This page walks through streaming Rerun recordings directly into a PyTorch `DataLoader`, without an intermediate export step, using the bundled [LeRobot ACT training example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader) end-to-end.
For an explanation of the dataloader API itself — windowed action chunks, GOP-aware video decoding, DDP partitioning — see [Train PyTorch models with Rerun](../howto/train.md).

> [!NOTE]
> The `rerun.experimental.dataloader` module is provisional and will change between releases.

## Run the example

The example trains a [LeRobot ACT](https://tonyzhaozh.github.io/aloha/) policy on the [`rerun/so101-pick-and-place`](https://huggingface.co/datasets/rerun/so101-pick-and-place) dataset from HuggingFace.

### 1. Grab the example

Sparse-checkout just the example directory, without the rest of the Rerun repo:

```bash
git clone --filter=blob:none --sparse https://github.com/rerun-io/rerun.git
cd rerun
git sparse-checkout set examples/python/dataloader
cd examples/python/dataloader
```

### 2. Install

The example has its own `uv` project because LeRobot pins an incompatible `rerun-sdk`.
The additional arguments to uv sync allow you to run just this example without the full rerun repo setup.

```bash
uv sync --no-sources --no-dev
```

If you have the full Rerun monorepo checked out and want to develop against your local Rerun build, run instead:

```bash
RERUN_ALLOW_MISSING_BIN=1 uv sync
uv pip install ../../../rerun_py/rerun_dev_fixup
```

### 3. Start a catalog server

In a separate terminal:

```bash
rerun server
```

### 4. Prepare and register the dataset

Downloads the dataset from HuggingFace, splits it into per-episode RRDs, and registers them with the catalog:

```bash
uv run python prepare_dataset.py
```

### 5. Train

```bash
uv run python train.py
```

The script streams batches from the catalog, trains an ACT policy for a few epochs, and saves a checkpoint to `act_checkpoint/`.

## References

- [Example source](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader) — `prepare_dataset.py` and `train.py`
- [`rerun/so101-pick-and-place`](https://huggingface.co/datasets/rerun/so101-pick-and-place) — LeRobot dataset on HuggingFace
- [Train PyTorch models with Rerun](../howto/train.md) — full how-to: windowing, video decoding, iterable vs. map style, DDP
