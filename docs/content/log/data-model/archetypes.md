---
title: Archetypes
order: 250
description: What an archetype is, what its name does, and how to pick one for your data
---

An archetype is a named set of semantically related [components](entity-component.md).
[`Points3D`](../../reference/types/archetypes/points3d.md), for example, groups positions, colors, and radii into a point cloud.
The name is stored with the data, and the Viewer uses it to choose how to show that data by default.

```python
rr.log("lidar", rr.Points3D(positions, colors=colors))
```

You log an archetype, and Rerun stores one component per field you set.
Here the `lidar` entity gets two: `Points3D:positions` and `Points3D:colors`.

The [archetype reference](../../reference/types/archetypes.md) lists every built-in archetype.

## What the archetype name does

### When logging

The archetype name becomes part of the column name.
`Points3D:colors` and `Boxes3D:colors` are both [`Color`](../../reference/types/components/color.md), but they are separate columns, so different archetypes can share an entity.

### In the Viewer

The archetype name selects the default [visualizer](../../viewer/view-concepts/customize-views.md), as long as the entity has the archetype's required fields.
An entity logged with `rr.Points3D` is drawn by the `Points3D` visualizer in a 3D view without any setup.
Data under another name is still available to that visualizer if its Arrow datatype matches, but in most views you add the visualizer yourself in the UI or the [blueprint](../../viewer/view-concepts/customize-views.md#choose-visualizers).
The exception is a time series view, which plots floating-point values logged under your own names.
See [Plot any scalar](../../viewer/view-types/time-series.md#plot-any-scalar).
The streams panel and the selection panel group an entity's components by archetype.

### In queries

The archetype name is part of each column name: `/lidar:Points3D:positions` is the entity path, the archetype, and the field.

## Required, recommended, and optional fields

Each archetype page sorts its fields into three groups.
**Required** fields are the core of the archetype, such as the `positions` of a `Points3D`.
The Viewer picks a default for any **recommended** or **optional** field you do not log.

Nothing checks these groups when you log.
You can log only the colors of a `Points3D` with `rr.Points3D.from_fields(colors=…)`, and the Viewer combines them with the latest positions.
See [Send partial updates over time](../send-data/send-partial-updates.md).

## Not every archetype is something you see

Most archetypes are data that a visualizer draws.
Some play a supporting role instead:

| Role | Examples |
|---|---|
| Placement | [`Transform3D`](../../reference/types/archetypes/transform3d.md), [`CoordinateFrame`](../../reference/types/archetypes/coordinate_frame.md), [`InstancePoses3D`](../../reference/types/archetypes/instance_poses3d.md), [`ViewCoordinates`](../../reference/types/archetypes/view_coordinates.md). [`Pinhole`](../../reference/types/archetypes/pinhole.md) places data too, and is also drawn as a camera frustum. See [Transforms & Coordinate Frames](transforms.md). |
| Styling | [`SeriesLines`](../../reference/types/archetypes/series_lines.md) and [`SeriesPoints`](../../reference/types/archetypes/series_points.md) style the [`Scalars`](../../reference/types/archetypes/scalars.md) on the same entity. [`StateConfiguration`](../../reference/types/archetypes/state_configuration.md) styles the [`StateChange`](../../reference/types/archetypes/state_change.md) on the same entity. |
| Interpretation | [`AnnotationContext`](../../viewer/view-concepts/annotation-context.md) holds the labels and colors for class IDs and keypoint IDs. |
| Bookkeeping | [`Clear`](../send-data/clears.md), [`RecordingInfo`](../../catalog/properties-and-segments.md), and the [MCAP](../../ingest/mcap/decoders-explained.md) archetypes. |

Most archetypes only affect the entity they are logged to.
A few also reach the entities below it in the [entity path hierarchy](entity-path.md), so that you log them once for a whole subtree:

- [`Transform3D`](../../reference/types/archetypes/transform3d.md) and [`Pinhole`](../../reference/types/archetypes/pinhole.md) apply to an entity and its children.
  See [Entity path transforms](transforms.md#entity-path-transforms).
- [`AnnotationContext`](../../reference/types/archetypes/annotation_context.md) applies to an entity and its children, unless a child has its own.
  See [Affected entities](../../viewer/view-concepts/annotation-context.md#affected-entities).
- A recursive [`Clear`](../../reference/types/archetypes/clear.md) clears an entity and its children.

## Choosing an archetype for your data

The archetype your data is stored under decides how the Viewer shows it by default and what its columns are called.

### A built-in archetype fits

The Viewer handles a built-in archetype without any setup.

When several fit, use the one that says the most about your data.
A pressure reading fits both [`Scalars`](../../reference/types/archetypes/scalars.md) and [`Measurements`](../../reference/types/archetypes/measurements.md), but only `Measurements` has fields for its variance and unit.

### No built-in archetype fits

Log the data under your own names.
A built-in archetype that means something else would label the columns with its own name and types.

`DynamicArchetype` groups values under an archetype name you choose, and `AnyValues` logs them without one:

| You log | Column |
|---|---|
| `rr.Points3D(positions)` | `Points3D:positions` |
| `rr.DynamicArchetype(archetype="GripperState", components={"position": 0.03})` | `GripperState:position` |
| `rr.AnyValues(confidence=0.9)` | `confidence` |

See [Send user-defined data](../send-data/custom-data.md) for both helpers.

You can query these columns like any other.
A time series view plots floating-point values without a mapping.
To draw anything else in a view, point a visualizer at it with a [component mapping](../../viewer/view-concepts/component-mappings.md).
To convert them into a built-in archetype, see [Lenses](../../query-and-transform/lenses.md).

### Only part of it fits

Log that part with the built-in archetype and the rest next to it on the same entity, for example a confidence per point next to a `Points3D`.
