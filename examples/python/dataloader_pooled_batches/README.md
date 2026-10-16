<!--[metadata]
title = "Pooled dataloader batches"
tags = ["API example", "Tensor"]
-->

`rerun.experimental.dataloader.PooledBatchCollator` reduces repeated CPU memory allocations when loading training batches with PyTorch.
Use it with `RerunIterableDataset`, `RerunMapDataset`, or another dataset that returns dictionaries.
It supports CPU tensors, NumPy arrays, numbers, and Rerun YUV frames, including data inside nested dictionaries, lists, and tuples.

Samples in a batch must have matching fields and structure, with consistent shapes and data types for each value.
YUV frames in the same field must also use the same color space and range.
Missing values and tensors that require gradients are not supported.

## Run the example

Build the Python SDK and run the example from the Rerun repository root:

```sh
pixi run py-build
pixi run uvpy examples/python/dataloader_pooled_batches/example.py
```

Pooling requires Linux and `multiprocessing_context="spawn"`, as shown below.
PyTorch must use its `file_descriptor` sharing strategy, which is the default on Linux.
With `num_workers=0`, the collator batches data without pooling and also works on other platforms.

## Use with the dataloader

Import the collator from `rerun.experimental.dataloader` in an environment with the Rerun SDK and PyTorch installed.
In the following example, replace `create_dataset()` and `train_step()` with your dataset setup and training code:

```python
from torch.utils.data import DataLoader

from rerun.experimental.dataloader import PooledBatchCollator


def main():
    dataset = create_dataset()
    loader = DataLoader(
        dataset,
        batch_size=96,
        num_workers=14,
        multiprocessing_context="spawn",
        persistent_workers=True,
        prefetch_factor=2,
        collate_fn=PooledBatchCollator(
            slots_per_field=4,
            max_pool_bytes=4 * 1024**3,
        ),
    )
    for batch in loader:
        train_step(batch)


if __name__ == "__main__":
    main()
```

Keep dataset creation and training inside `main()` so worker processes can import the script without starting another training loop.

## Memory use

Memory is allocated as needed rather than reserving the full limit upfront.
You can safely keep a batch, tensor slice, or NumPy view while loading more data.
If all reusable buffers are still in use, the collator allocates new ones.
Clone tensors or YUV frames with `.clone()` before sending them to another process.

Two settings control how much memory each worker keeps for reuse:

- `slots_per_field`: maximum cached buffers for each tensor in a sample.
- `max_pool_bytes`: maximum total cached memory **per worker**.

These limits do not cap total DataLoader memory: prefetched batches and batches you keep can use additional memory.
The example above allows up to 56 GiB of cached memory across 14 workers, so adjust these settings for your available RAM.
Your machine or container also needs enough shared memory (`/dev/shm`) for cached and in-flight batches.
