---
title: "Assets can pick which segments they apply to"
hidden: true
type: feature
---

### Assets can specify what segments they apply to

// TODO(isse): Add video when viewer UI is merged.

`register_asset` now takes a `mode` and a list of `segments`, so an asset no longer has to apply to every segment of a dataset.
The default `mode="opt_out"` applies the asset to every segment except the ones listed, while `mode="opt_in"` applies it only to the ones listed.

```python
dataset.register_asset("s3://…/mesh.rrd", mode="opt_in", segments=["episode_0", "episode_1"])
```

Which segments an asset applies to can be changed afterwards with `add_segments_to_asset` and `remove_segments_from_asset`.

Use `assets_for_segment` to get a list of assets that apply to a segment.

See [assets](../concepts/query-and-transform/catalog-object-model.md#assets) for what an asset is.
