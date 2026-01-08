---
title: Send partial updates over time
order: 200
description: How to use the Rerun SDK to send partial data updates over time
---

Rerun allows you to log only the data that has changed in-between frames (or whatever atomic unit [your timeline](../../concepts/timelines.md) is using), i.e. you can think of this as a sort of diffs or delta encodings.

This is a natural consequence of how Rerun [ingests, stores](../../concepts/chunks.md) and finally [queries](../../reference/entity-queries.md) data: Rerun *always* operates that way, whether you're aware of it or not. Consider this simple snippet:

snippet: archetypes/points3d_simple

Here, only the positions of the points have been specified but, looking at the [complete definition for Points3D](../../reference/types/archetypes/points3d.md), we can see that it has quite a few more [components](../../concepts/entity-component.md#data-model) available:
> **Required**: [`Position3D`](../../reference/types/components/position3d.md)
>
> **Recommended** & **Optional**: [`Radius`](../../reference/types/components/radius.md), [`Color`](../../reference/types/components/color.md), [`Text`](../../reference/types/components/text.md), [`ShowLabels`](../../reference/types/components/show_labels.md), [`ClassId`](../../reference/types/components/class_id.md), [`KeypointId`](../../reference/types/components/keypoint_id.md)


All three languages for which we provide logging SDKs (Python, Rust, C++) expose APIs that allow fine-grained control over which components of an archetypes, when, and how.

The best way to learn about these APIs is to see them in action: check out the examples below.


## Examples


### Update specific properties of a point cloud over time

snippet: archetypes/points3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/points3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1200w.png">
  <img src="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/full.png">
</picture>


### Update specific properties of a transform over time

snippet: archetypes/transform3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/transform3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1200w.png">
  <img src="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/full.png">
</picture>


### Update specific parts of a 3D mesh over time

snippet: archetypes/mesh3d_partial_updates

<picture data-inline-viewer="snippets/archetypes/mesh3d_partial_updates">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/1200w.png">
  <img src="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/full.png">
</picture>
