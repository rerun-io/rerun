---
title: "Experimental iterable dataloader skips missing samples"
hidden: true
type: feature
---

### Experimental iterable dataloader skips missing samples

The live `RerunIterableDataset` now skips samples when a decoder returns `None`, warning once per missing field.
`NumericDecoder` returns `None` for a window whose rows have inconsistent widths, allowing the affected live sample to be skipped instead of stopping iteration.
Null source rows and expected encoded-image or video decode failures also resolve to `None`, with video failures isolated to the affected GOP.
Filtering happens before the optional emission shuffle so incomplete samples do not occupy its buffer.
The `max_consecutive_skipped_samples` option defaults to 100 and caps the number skipped in a row by each rank and `DataLoader` worker before iteration raises with total and per-field counts; pass `None` to disable the limit.
Because missing samples are skipped after rank sharding, finite DDP training loops must use `DistributedDataParallel.join()` so one rank finishing early does not stall the others.

Manifest replay remains strict: if a field recorded as required no longer resolves, iteration raises an error asking the user to regenerate the manifest rather than silently changing its frozen sample order.
Valid zero-sized tensors are preserved; only `None` denotes missing data.

Docs: ../howto/train/dataloader.md
