---
title: Show a subset of plotted series
order: 230
description: Hide individual series of a multi-value scalar plot
---

When you log several values in one [`Scalars`](../../reference/types/archetypes/scalars.md) batch, such as the joint positions of a robot arm, each value is plotted as its own series in a [TimeSeriesView](../../reference/types/views/time_series_view.md).
You can show only some of them from the Viewer or from code.

## In the Viewer

Click a series in the plot legend to hide or show it.
The Viewer stores this choice in the blueprint.

## From code

Set `visible_series` of [`SeriesLines`](../../reference/types/archetypes/series_lines.md) as a blueprint override.
It takes one boolean per series, in the order of the values in the batch.
If the list is shorter than the number of series, its last value is repeated.
Hidden series stay in the legend, so you can click them to show them again.
For series drawn as points, use [`SeriesPoints`](../../reference/types/archetypes/series_points.md) instead.

snippet: howto/visible_series
