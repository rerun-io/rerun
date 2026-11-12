---
title: Some of my data is not shown
order: 300
description: Why logged data, labels, or plot series can be missing, and how to fix it
---

If you logged something but can't see it in the Viewer, go through the list below.
First, check that the data arrived at all: look for the entity in the streams panel at the bottom.

## The time cursor is on another timeline

Data is only shown on the timelines it was logged on.
If you log with `rr.set_time("frame", sequence=…)` but the Viewer shows another custom timeline, for example `sensor_time`, the data appears to be missing.
By default, the SDK also logs all data on the automatic `log_time` timeline, so `log_time` shows it unless you turned that timeline off.
Pick the right timeline in the time panel.
Static data is shown on every timeline.
See [timelines](../../logging-and-ingestion/data-model/timelines.md).

## The time cursor is before the first data point

The Viewer shows the latest value at or before the time cursor.
If the cursor is before the first time an entity was logged, the entity has no value yet and is not shown.
Move the time cursor forward, or press play.

## The entity is not part of any view

Views only show the entities that their [entity query](../blueprints-and-views/entity-queries.md) includes.
If you changed the query, or set up the layout with a blueprint, new entities may not match it.
Select the view and check its entity query in the selection panel, or right-click the entity in the streams panel and pick **Add to new view**.

When the Viewer creates views automatically, it creates at most 8 views of each kind (for example 8 time series views).
If your data needs more, add the views yourself in the UI or with a [blueprint](../blueprints-and-views/build-a-blueprint-programmatically.md).

## The entity is hidden

An entity can be hidden in a view with the eye icon in the blueprint panel.
Check that it is visible.

## The view can't place it in space

In 2D and 3D views, an entity whose transform frame isn't connected to the view's origin can't be drawn.
The view shows an error for it in the selection panel.
See [Debugging transform trees](../../logging-and-ingestion/data-model/transforms.md#debugging-transform-trees).

Points that are too small to see, or 2D elements hidden behind others, are covered in [Rendering issues](../../getting-started/install-rerun/troubleshooting.md#rendering-issues).

## Labels are not shown

For points, boxes, arrows, line strips, and similar shapes, labels are hidden by default when an entity has 30 or more instances, to avoid clutter.
A single label for the whole entity is always shown.
To always show labels, set `show_labels=True`:

```python
rr.log("points", rr.Points3D(positions, labels=names, show_labels=True))
```

## Some plot series are missing

When you plot a component that isn't logged as `Scalars` (see [Plot any scalar](../plots-and-maps/plot-any-scalar.md)), a time series view shows at most 100 series per entity.
Log the values as `Scalars` to plot more.

## Old data disappeared

When the Viewer reaches its memory limit, it drops data to stay under it, oldest first for live data.
See [Limit the Viewer's memory usage](limit-ram.md).

## The data is in another recording

Data with a different application ID or recording ID goes to a separate recording.
Check the recording panel on the left and select the right recording.
If several processes log to the same recording, they need the same application ID and recording ID, see [Share recordings across multiple processes](../../logging-and-ingestion/sinks-and-modes/shared-recordings.md).
