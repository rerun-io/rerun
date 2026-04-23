Train a [LeRobot](https://github.com/huggingface/lerobot) ACT policy using Rerun's experimental PyTorch dataloader, streaming trajectory data directly from a Rerun Data Platform catalog.

## Background

The Rerun Data Platform stores multimodal robot data (video streams, scalar signals, poses, …) as time-indexed recordings.
The `rerun.experimental.dataloader` module exposes those recordings as a PyTorch-style `Dataset`, so you can plug them straight into a standard `DataLoader` and training loop.

This example shows how to:

- register a LeRobot dataset (from HuggingFace Hub) to a local Rerun Data Platform instance
- build a `RerunDataset` that decodes video frames and scalar columns on the fly
- use the `Column.window` feature to fetch future action chunks in a single query per batch
- train an [ACT](https://tonyzhaozh.github.io/aloha/) (Action Chunking Transformer) policy on the resulting batches

## Run the code

### 1. Install dependencies

This example has its own `uv` project, separate from the workspace `.venv`, because LeRobot pins an
incompatible `rerun-sdk`. From the repo root:

```bash
cd examples/python/dataloader
RERUN_ALLOW_MISSING_BIN=1 uv sync  # builds local rerun-sdk + installs lerobot into ./.venv
```

Then either `source .venv/bin/activate` or prefix subsequent commands with `uv run`.

### 2. Start a local Rerun server

In a separate terminal:

```bash
rerun server
```

This serves a Rerun server at `rerun+http://127.0.0.1:51234` (the default used by the scripts).

### 3. Prepare and register the dataset

Downloads a LeRobot dataset from HuggingFace, splits it into per-episode RRDs, and registers them as a dataset in the catalog:

```bash
uv run python prepare_dataset.py
```

Pass `--repo-id user/other_lerobot_ds` to use a different dataset, or `--catalog-url ""` to skip registration and only write local RRDs.

### 4. Train

```bash
uv run python train.py
```

The script streams batches from the catalog, trains an ACT policy for a few epochs, and saves a checkpoint to `act_checkpoint/`.

It accepts a few CLI flags (run `uv run python train.py --help` for the full list):

```bash
uv run python train.py \
    --catalog-url rerun+http://127.0.0.1:51234 \
    --dataset rerun_so101-pick-and-place \
    --num-segments 3 \
    --epochs 5 \
    --batch-size 8 \
    --num-workers 8 \
    --lr 1e-5 \
    --checkpoint-dir act_checkpoint
```

Pass `--num-segments 0` to train on all segments in the dataset.

### 4b. Train with traces
TELEMETRY_ENABLED=true OTEL_SDK_ENABLED=true uv run python train.py
