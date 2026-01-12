---
title: Query semantics & partial updates
order: 800
---

## The Rerun data model is based around streams of entities with components

In Rerun, you model your data using entities (roughly objects) with [batches of components](batches.md) that change over time.
An entity is identified by an entity path, e.g. `/car/lidar/points`, where the path syntax can be used to model hierarchies of entities.
A point cloud could be made up of positions and colors, but you can add whatever components you like to the entity.
Point positions are e.g. represented as a batch of `Position3D` component instances.

<picture>
  <img src="https://static.rerun.io/data-model/f64dfbf4a9aa09c9765508d49de6e35e8b76d159/full.png" alt="A diagram showing an overview of the Rerun data model">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/data-model/f64dfbf4a9aa09c9765508d49de6e35e8b76d159/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/data-model/f64dfbf4a9aa09c9765508d49de6e35e8b76d159/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/data-model/f64dfbf4a9aa09c9765508d49de6e35e8b76d159/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/data-model/f64dfbf4a9aa09c9765508d49de6e35e8b76d159/1200w.png">
</picture>

Components can have different values for different times, and do not have to be updated all at once.
Rerun supports multiple timelines (sequences of times), so that you can explore your data organized according to e.g. the camera's frame index or the time it was logged.


## Core queries

All data that gets sent to the Rerun viewer is stored in an in-memory database, and there are two core types of queries against the database that visualizers in the viewer run.

<picture>
  <img src="https://static.rerun.io/latest-at/2720caee9646a09792cc8fd71ad50503f1cf4dcd/full.png" alt="A diagram showing an overview of a latest-at query in Rerun">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/latest-at/2720caee9646a09792cc8fd71ad50503f1cf4dcd/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/latest-at/2720caee9646a09792cc8fd71ad50503f1cf4dcd/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/latest-at/2720caee9646a09792cc8fd71ad50503f1cf4dcd/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/latest-at/2720caee9646a09792cc8fd71ad50503f1cf4dcd/1200w.png">
</picture>

**Latest-at queries** collect the latest version of each of an entity's components at a particular time.
This allows the visualizer to draw the current state of an object that was updated incrementally.
For example, you might want to update the vertex positions of a mesh while keeping textures and triangle indices constant.

<picture>
  <img src="https://static.rerun.io/range/cbe71efc5afe21135568c07cc6381306a3057fde/full.png" alt="A diagram showing an overview of a range query in Rerun">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/range/cbe71efc5afe21135568c07cc6381306a3057fde/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/range/cbe71efc5afe21135568c07cc6381306a3057fde/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/range/cbe71efc5afe21135568c07cc6381306a3057fde/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/range/cbe71efc5afe21135568c07cc6381306a3057fde/1200w.png">
</picture>

**Range queries** instead collect all components associated with times on a time range.
These queries drive any visualization where data from more than one time is shown at the same time.
The obvious example is time series plots,
but it can also be used to e.g. show lidar point clouds from the last 10 frames together.

The queried range is typically configurable, see for instance [this how-to guide on fixed windows plots](../../howto/visualization/fixed-window-plot.md) for more information.


## Partial updates

As mentioned above, the query semantics that power the Rerun Viewer, coupled with our [chunk-based storage](chunks.md), make it possible to log only the components that have changed in-between frames (or whatever atomic unit [your timeline](timelines.md) is using).

Here's an example of updating only some specific properties of a point cloud, over time:

snippet: archetypes/points3d_partial_updates

To learn more about how to use our partial updates APIs, refer to [this page](../../howto/logging/send-partial-updates.md).
