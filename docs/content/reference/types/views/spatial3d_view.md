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

Whenever no visual time range applies, queries are done with "latest at" semantics.
This means that the view will, starting from the time cursor position,
query the latest data available for each component type.

The default visual time range depends on the type of view this property applies to:
- For time series views, the default is to show the entire timeline.
- For any other view, the default is to apply latest-at semantics.

* ranges: The time ranges to show for each timeline unless specified otherwise on a per-entity basis.

## Links
 * 🐍 [Python API docs for `Spatial3DView`](https://ref.rerun.io/docs/python/stable/common/blueprint_views#rerun.blueprint.views.Spatial3DView)

## Example

### Use a blueprint to customize a Spatial3DView.

snippet: views/spatial3d

<picture data-inline-viewer="snippets/spatial3d">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/spatial3d_view/9f21738776c759e6c6ef2fd0732333509ecbbcc5/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/spatial3d_view/9f21738776c759e6c6ef2fd0732333509ecbbcc5/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/spatial3d_view/9f21738776c759e6c6ef2fd0732333509ecbbcc5/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/spatial3d_view/9f21738776c759e6c6ef2fd0732333509ecbbcc5/1200w.png">
  <img src="https://static.rerun.io/spatial3d_view/9f21738776c759e6c6ef2fd0732333509ecbbcc5/full.png">
</picture>

