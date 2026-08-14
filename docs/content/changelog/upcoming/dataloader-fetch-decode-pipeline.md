---
title: "Experimental dataloader decoders now process fetch blocks"
hidden: true
type: breaking
---

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

Docs: ../howto/train/dataloader.md
