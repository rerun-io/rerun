---
title: Migrating from 0.12 to 0.13
order: 996
---

## `TimeSeriesScalar` deprecated in favor of [Scalar](../types/archetypes/scalar.md) & [SeriesLine](../types/archetypes/series_line.md)/[SeriesPoint](../types/archetypes/series_point.md)

Previously, `TimeSeriesScalar` was used to define both
data and styling of time series plots.
Going forward, this is done separately: data is now logged via [Scalar](../types/archetypes/scalar.md).
Styling for point/marker series via [SeriesPoint](../types/archetypes/series_point.md) and styling for
line series via [SeriesLine](../types/archetypes/series_line.md).
(Both styling archetypes are typically logged as `timeless` but this is not a requirement and any property may change over time!)

`TimeSeriesScalar` will be removed in a future release.

## Changes in space view creation heuristics

The overhaul of automatic Space View creation makes the Viewer faster and
more predictable but comes with a few changes on how paths are expected to be structured:

* When working with images of different resolutions, the image entities will end up defining the root of the created spaces.
  * This means shapes like annotated rects that are in image coordinates are best logged **below** rather than next-to
    the image path.
  * Example:
    * Before: image at `image/rgb`, rects at `image/annotation`
    * After: image at `image`, rects at `image/annotation`
  * Previously Rerun treated children of the root-space as special. This behavior has been removed for 2D views to
    give more predictable results regardless of prefix.
    * The primary 2D partitioning is now driven by putting images of the same size in the same view, with each space
      being created as the common ancestor of all the images of the size.
    * As such, it is important to put any non-image 2D content in a location that will be included in the space of
      the appropriate dimensions.
    * Note that children of root are still special for 3D & time series views but this may change in the future
      see [#4926](https://github.com/rerun-io/rerun/issues/4926)
* `DisconnectedSpace` now strictly applies only to 2D and 3D Space Views
  * Internally, the heuristic now reasons about a 2D/3D topology which does not affect other types of views.
    `DisconnectedSpace` represents a hard cut in this topology.

Future releases will allow you to specify Space Views & view layout from code.
