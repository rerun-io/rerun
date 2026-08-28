---
title: "Assets"
hidden: true
type: feature
---

### Assets

[Assets](../concepts/query-and-transform/catalog-object-model.md#assets) are static data registered on a dataset and shared by its segments, such as a robot mesh that every episode refers to.

When you open a segment that has an asset the viewer downloads the asset, and caches it so other segments in the dataset don't
have to download the same asset again.


When viewing a dataset in the viewer, one can also list assets and certain metadata about them. As well as view them individually:

// TODO(isse): Add video.

Assets can also be registered, and unregistered right in the viewer:

// TODO(isse): Add video.

From Python, a segment store covers the dataset's assets alongside the segment's own data.
Pass `include_assets=False` to leave them out, which also skips the requests for their manifests:

```python
dataset.register_asset("file:///data/robot_mesh.rrd")

# Describes the chunks of both the segment and the asset.
store = dataset.segment_store(segment_id)
for chunk in store.stream().to_chunks():
    print(chunk.entity_path)

# Describes only the segment, no asset manifests are fetched.
segment_only = dataset.segment_store(segment_id, include_assets=False)
```

// TODO(isse): Add snippet on assets, and how to register them.
