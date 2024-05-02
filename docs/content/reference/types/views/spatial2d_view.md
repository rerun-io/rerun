---
title: "Spatial2DView"
---

A Spatial 2D view.

## Properties

### `Background`
Configuration for the background of a view.

* kind: The type of the background. Defaults to BackgroundKind.GradientDark.
* color: Color used for BackgroundKind.SolidColor.
### `VisualBounds`
Controls the visual bounds of a 2D space view.

* range2d: The visible parts of a 2D space view, in the coordinate space of the scene.
### `VisibleTimeRanges`
Configures what range of each timeline is shown on a view.

Refer to [`VisibleTimeRange`] component for more information.

* ranges: The ranges of time to show for all given timelines based on sequence numbers.

## Links
 * 🐍 [Python API docs for `Spatial2DView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.Spatial2DView)

