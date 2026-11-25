---
title: Text labels in 2D and 3D
order: 600
description: Label instances and entities in 2D and 3D views
---

This page covers label placement, visibility, and limitations in 2D and 3D views.

## Add labels

Many 2D and 3D archetypes have a `labels` field:
[`Points2D`](../../reference/types/archetypes/points2d.md),
[`Points3D`](../../reference/types/archetypes/points3d.md),
[`Boxes2D`](../../reference/types/archetypes/boxes2d.md),
[`Boxes3D`](../../reference/types/archetypes/boxes3d.md),
[`Arrows2D`](../../reference/types/archetypes/arrows2d.md),
[`Arrows3D`](../../reference/types/archetypes/arrows3d.md),
[`LineStrips2D`](../../reference/types/archetypes/line_strips2d.md),
[`LineStrips3D`](../../reference/types/archetypes/line_strips3d.md),
[`Ellipses2D`](../../reference/types/archetypes/ellipses2d.md),
[`Ellipsoids3D`](../../reference/types/archetypes/ellipsoids3d.md),
[`Capsules3D`](../../reference/types/archetypes/capsules3d.md),
and [`Cylinders3D`](../../reference/types/archetypes/cylinders3d.md).

```python
rr.log(
    "detections",
    rr.Boxes2D(
        mins=[[10, 10], [80, 40]],
        sizes=[[50, 30], [40, 40]],
        labels=["car", "person"],
    ),
)
```

- With one label per instance, each instance gets its own label.
- With a single label, it is placed once, at the center of the entity.

Labels can also come from an [annotation context](../view-concepts/annotation-context.md), through class IDs.

## Why are my labels not shown?

By default, labels are only shown when the entity has a single label, or when it has few instances.
This keeps large point clouds readable.
To always show them, set `show_labels=True`:

```python
rr.log("points", rr.Points3D(positions, labels=names, show_labels=True))
```

You can also toggle `show_labels` in the Viewer: select the entity and change it in the visualizer section of the selection panel.

## A standalone text label

There is no archetype for text alone.
To place a label at a position in 2D or 3D, log a single point with a label, and make the point small:

```python
rr.log("world/label", rr.Points3D([[1.0, 2.0, 0.5]], labels="Start", radii=0.001))
```

## Limitations

Labels are drawn at a fixed screen size.
You cannot change the font size or the font, and labels do not scale with the zoom level.
See [missing features](../../development/roadmap.md#missing-features).
