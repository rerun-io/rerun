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

Whenever no visual time range applies, queries are done with "latest at" semantics.
This means that the view will, starting from the time cursor position,
query the latest data available for each component type.

The default visual time range depends on the type of view this property applies to:
- For time series views, the default is to show the entire timeline.
- For any other view, the default is to apply latest-at semantics.

* ranges: The time ranges to show for each timeline unless specified otherwise on a per-entity basis.

## Links
 * 🐍 [Python API docs for `Spatial2DView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.Spatial2DView)

## Example

### Use a blueprint to customize a Spatial2DView.

snippet: views/spatial2d

<picture data-inline-viewer="snippets/spatial2d">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/spatial2d/074c0822870325d6502c9f51c165c1181a20e83f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/spatial2d/074c0822870325d6502c9f51c165c1181a20e83f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/spatial2d/074c0822870325d6502c9f51c165c1181a20e83f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/spatial2d/074c0822870325d6502c9f51c165c1181a20e83f/1200w.png">
  <img src="https://static.rerun.io/spatial2d/074c0822870325d6502c9f51c165c1181a20e83f/full.png">
</picture>

