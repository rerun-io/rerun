Train a [LeRobot](https://github.com/huggingface/lerobot) ACT policy using Rerun's experimental PyTorch dataloader, streaming trajectory data directly from a Rerun catalog.

For an explanation of the dataloader API and how the example fits together, see the [Train PyTorch models with the Rerun dataloader](https://rerun.io/docs/howto/integrations/dataloader?speculative-link) how-to guide.

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
    --checkpoint-dir act_checkpoint \
    --dataset-style iterable  # or "map"
```

Pass `--num-segments 0` to train on all segments in the dataset.

### Training with traces

```sh
TELEMETRY_ENABLED=true OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4317 uv run python train.py
```
