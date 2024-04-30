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
### `VisibleTimeRange`
Configures what range of the timeline is shown on a view.

Whenever no visual time range applies, queries are done with "latest at" semantics.
This means that the view will, starting from the time cursor position,
query the latest data available for each component type.

The default visual time range depends on the type of view this property applies to:
- For time series views, the default is to show the entire timeline.
- For any other view, the default is to apply latest-at semantics.

The visual time range can be overridden also individually per entity.

* sequence: The range of time to show for timelines based on sequence numbers.
* time: The range of time to show for timelines based on time.

## Links
 * 🐍 [Python API docs for `Spatial2DView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.Spatial2DView)

