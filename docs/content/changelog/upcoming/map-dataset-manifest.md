---
title: "`RerunMapDataset` can be built from a manifest"
hidden: true
type: feature
---

### `RerunMapDataset` can be built from a manifest

`RerunMapDataset.from_manifest(manifest, source, fields)` builds a map-style dataset over the validated samples of a frozen `Manifest`, mirroring `RerunIterableDataset.from_manifest`.

This is a performance optimization: the manifest already holds the validated sample set and each field's frozen decode range, so the dataset skips the live scan at construction and the per-batch keyframe lookup when fetching.

```python
manifest = Manifest.from_parquet("epoch.parquet")
dataset = RerunMapDataset.from_manifest(manifest, source, fields)
loader = DataLoader(dataset, batch_size=8, sampler=DistributedSampler(dataset))
```

The manifest's recorded order is **not** replayed here.
Ordering and cross-worker sharding stay with the `DataLoader`'s sampler, as for any map-style dataset, so this cannot reproduce a manifest's run, use `RerunIterableDataset.from_manifest` for reproducible, resumable training.

Docs: ../howto/train/dataloader.md
