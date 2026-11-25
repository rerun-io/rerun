---
title: Pack pooled batches for single-copy pinning
hidden: true
type: breaking
---

### Pack pooled batches for single-copy pinning

Use `PooledBatchCollator(packed=True)` to combine tensor fields into one shared CPU buffer and pin them with one copy, while consuming dictionary-compatible batches through the standard PyTorch DataLoader.
The dataloader now requires PyTorch 2.11 or newer instead of 2.5, including per-field collation (`packed=False`).
Upgrade with `pip install --upgrade 'rerun-sdk[dataloader]'`.
See the [dataloader guide](../train/dataloader.md) and [pooled-batch example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader_pooled_batches).
