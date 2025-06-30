---
title: Migrating from 0.23 to 0.24
order: 986
---
<!--   ^^^ this number must be _decremented_ when you copy/paste this file -->

## Changed timeline navigation keyboard shortcut

To accommodate the new tree keyboard navigation feature, the timeline navigation is changed as follows:

- go to previous/next frame is ctrl-left/right (cmd on Mac) arrow (previously no modifier was needed)
- go to beginning/end of timeline is alt-left/right (previously the ctrl/cmd modifier was used)

## Previously deprecated, now removed

### `Scalar`, `SeriesLine`, `SeriesPoint` archetypes

Have been removed in favor of `Scalars`, `SeriesLines`, `SeriesPoints` respectively.


## Combining `InstancePoses3D` with orientations in `Boxes3D`/`Ellipsoids3D`/`Capsules3D` behaves differently in some cases now

Previously, `Boxes3D`/`Ellipsoids3D`/`Capsules3D` all mirrored some transform components from `InstancePoses3D`.
However, now that all components have distinct archetype-tags from the transform components in `InstancePoses3D`, form a separate transform
that is applied prior to `InstancePoses3D`.

I.e. transform resolve order was previously:
```
final = `Transform3D` * … * `Transform3D` * ([Box3D or `InstancePoses3D].quaternion * [Box3D or `InstancePoses3D].rotation_axis_angle * [Box3D or `InstancePoses3D].centers/translations)
```
And is now
```
final = `Transform3D` * … * `Transform3D` * InstancePoses3D * (Box3D.quaternion * Box3D.rotation_axis_angle * Box3D.centers)
```

As a concrete example, if you previously scaled boxes/ellipsoids/capsules using `InstancePoses3D` they would be scaled relative to the individual box centers.
Now instead they are scaled relative to the entity's center.

## `serve_web` is now deprecated in Rust and Python

`serve_web` will be removed in a future release but is still available for now.
Instead prefer an explicit combination of `serve_grpc` and `serve_web_viewer`:

snippet: howto/serve_web_viewer

Rust's even older `serve` (deprecated since 0.20) has been removed entirely now.

## Component descriptors

One limitation that we previously had with our data model was that it was only possible to use any `Component` once per entity path.
This severely effected the design and flexbility of our archetypes.
The underlying reason for this was that the our internal datastructures used [`Component`] to index into the data.
This release changes how components are identified within the viewer and within our APIs:
Instead of specifying the [`Component`] name, components are now referenced by a new syntax that consists of the (short) archetype name + the archetype field name separated by a colon (`:`).

As an example, `Points3D:positions` refers to the `positions` component (`rerun.components.Position3D`) in the `rerun.archetypes.Points3D` archetype.
For custom data, such as `AnyValues`, it is possible to omit the archetype part of this syntax and only specify the fied name.

Internally we use, what we call a `ComponentDescriptor` to uniquely identify a component.
Conceptually, it looks like the following:

```rs
struct ComponentDescriptor {
  archetype:      Option<String>, // e.g. `rerun.archetypes.Points3D
  component:      String,         // e.g. `Points3D:positions`
  component_type: Option<String>, // e.g. `rerun.components.Position3D
}
```

### Changes

When logging data to Rerun using the builtin archetypes no changes to user code should be required.
There is also migration code in place so that you can open `.rrd` files that were created with `v0.23`.

These changes are reflected in various parts of Rerun:

* The selection panel UI comes with a revamped display of archetypes that uses the new syntax to show the `ComponentDescriptor` for each component.
* The new `:`-based syntax needs to be used when referring to components in the dataframe API and in the dataframe view.
* Custom data can still use simple field names such as `confidences`, but it is advised to supply a full `ComponentDescriptor`, if possible.

### Limitations

* In rare cases, it is not possible to migrate previous blueprints, _but only if they were saved from the viewer via the UI.
* Currently, only Rerun-builtin components are picked up by the visualizers and therefore shown in the views (except for the dataframe view which shows all components).
* In `v0.23`, the LeRobot dataloader logged incomplete `ComponentDescriptors` for robot observations and actions. To fix this, load the dataset in `v0.24` and resave your episodes to `.rrd` (`v0.24` now supports saving all selected recordings).

## Dataframe API: `View.select_static` is deprecated

The dataframe API was originally introduced with the `View.select_static()` variant of `View.select()` used when the view only contains static columns. In such cases, `select()` would yield no row as no data is logged at any index value (by definition of static-only content). Instead, `select_static()` would force the generation of a single, index-less row populated with the static data.

The same behavior can now be achieved by using `Recording.view(index=None, content=...)`.
