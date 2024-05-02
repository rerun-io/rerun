---
title: "Spatial3DView"
---

A Spatial 3D view.

## Properties

### `Background`
Configuration for the background of a view.

* kind: The type of the background. Defaults to BackgroundKind.GradientDark.
* color: Color used for BackgroundKind.SolidColor.
### `VisibleTimeRanges`
Configures what range of each timeline is shown on a view.

Refer to [`VisibleTimeRange`] component for more information.

* ranges: The ranges of time to show for all given timelines based on sequence numbers.

## Links
 * 🐍 [Python API docs for `Spatial3DView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.Spatial3DView)

## Example

### Use a blueprint to customize a Spatial3DView

snippet: spatial3dview

