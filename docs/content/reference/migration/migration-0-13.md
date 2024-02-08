---
title: Migrating from 0.12
order: 11
---

## [TimeSeriesScalar](../types/archetypes/time_series_scalar.md) scalar deprecated in favor of [Scalar](../types/archetypes/scalar.md) & [SeriesLine](../types/archetypes/series_line.md)/[SeriesPoint](../types/archetypes/series_point.md)

Previously, [TimeSeriesScalar](../types/archetypes/time_series_scalar.md) was used to define both
data and styling of time series plots.
Going forward, this is done separately: data is now logged via [Scalar](../types/archetypes/scalar.md).
Styling for point/marker series via [SeriesPoint](../types/archetypes/series_point.md) and styling for
lien series via [SeriesLine](../types/archetypes/series_line.md).
(Both series archetypes are typically logged as `timeless` but this is not a requirement and all properties may change over time!)

[TimeSeriesScalar](../types/archetypes/time_series_scalar.md) will be removed in a future release.

## Changes in Space View creation heuristics

The overhaul of automatic Space View creation makes the viewer faster and
more predictable but comes with a few changes on how paths are expected to be structured:

* when expecting several 2D views, log annotations **below** image paths
  * Example:
    * Before: image @ `image/rgb`, rects at `image/annotation`
    * After: image `image`, rects at `image/annotation`
  * This happens because children of roots are no longer treated special for 2D views, but the viewer still
    tries to bucket by image, putting images of the same size in the same view
    * meaning the viewer no longer breaks up the root unless image-based bucketing implies it
    * note that children of root are still special for 3D & time series views but this may change in the future
      see [#4926](https://github.com/rerun-io/rerun/issues/4926)
* [DisconnectedSpace](../types/archetypes/disconnected_space.md) now strictly applies only to 2D and 3D Space Views
  * Internally, the heuristic reasons now about a 2D/3D topology which does not affect other types of views.
    [DisconnectedSpace](../types/archetypes/disconnected_space.md) represents a hard cut in this topology.

Future releases will allow you to specify Space Views & view layout from code.
