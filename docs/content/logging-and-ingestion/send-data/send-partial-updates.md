---
title: Send partial updates over time
order: 200
description: Log only the components that change from one frame to the next
---

Rerun allows you to log only the data that has changed in-between frames (or whatever atomic unit [your timeline](../data-model/timelines.md) is using), i.e. you can think of this as a sort of diffs or delta encodings.

This is a natural consequence of how Rerun [ingests, stores](../chunks-and-rrd/chunks.md) and finally [queries](../../viewer/blueprints-and-views/entity-queries.md) data: Rerun *always* operates that way, whether you're aware of it or not. Consider this simple snippet:

snippet: archetypes/points3d_simple

Here, only the positions of the points have been specified but, looking at the [complete definition for Points3D](../../reference/types/archetypes/points3d.md), we can see that it has quite a few more [components](../data-model/entity-component.md#data-model) available:
> **Required**: [`Position3D`](../../reference/types/components/position3d.md)
>
> **Recommended** & **Optional**: [`Radius`](../../reference/types/components/radius.md), [`Color`](../../reference/types/components/color.md), [`Text`](../../reference/types/components/text.md), [`ShowLabels`](../../reference/types/components/show_labels.md), [`PointShading`](../../reference/types/components/point_shading.md), [`ClassId`](../../reference/types/components/class_id.md), [`KeypointId`](../../reference/types/components/keypoint_id.md)


### Log only what changed

The SDK does not deduplicate: if you log the same value again, it is stored again.
To save bandwidth and memory, skip values that did not change yourself.
The Viewer uses [latest-at semantics](../data-model/latest-at.md): each component keeps its last logged value until you log a new one, so values you skip carry forward.

```python
if not np.array_equal(colors, last_colors):
    rr.log("points", rr.Points3D.from_fields(colors=colors))
    last_colors = colors.copy()  # Copy, in case `colors` is updated in place.
rr.log("points", rr.Points3D.from_fields(positions=positions))
```

All three languages for which we provide logging SDKs (Python, Rust, C++) expose APIs that allow fine-grained control over which components of an archetype are logged, when, and how.

The best way to learn about these APIs is to see them in action: check out the examples below.


## Examples


### Update specific properties of a point cloud over time

snippet: archetypes/points3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/points3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1200w.png">
  <img src="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/full.png" alt="3D view of a row of ten points above the ground grid, the first six large and green, the last four small and red">
</picture>


### Update specific properties of a transform over time

snippet: archetypes/transform3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/transform3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1200w.png">
  <img src="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/full.png" alt="3D view of a tilted magenta box with red, green and blue axis arrows through it, paused midway on the log_time timeline">
</picture>


### Update specific parts of a 3D mesh over time

snippet: archetypes/mesh3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/mesh3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/1200w.png">
  <img src="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/full.png" alt="3D view of a triangle mesh with red, green and blue vertex colors, with a hover tooltip for /triangle listing its components">
</picture>
