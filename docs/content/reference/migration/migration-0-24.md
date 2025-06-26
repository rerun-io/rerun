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

## Tagged components

<!-- TODO(grtlr): These are ad-hoc notes from https://github.com/rerun-io/rerun/pull/10082 and need to be cleaned up! -->

#### `re_types_core`

* The obvious `ComponentDescriptor` changes.

#### `re_types`

* Indicator components now become indicator fields. This is only temporary until we get rid of indicators.

#### `re_log_types`

* Removed auto-filling of `rerun.components.` to `ComponentType` in path parsing.

#### `re_sorbet`

* Lookup `ColumnDescriptor` by `ComponentIdentifier` instead of `ComponentType`.
* Changed `ComponentColumnSelector`.
* Changed `ComponentColumnDescriptor::column_name` to use fully-qualified column names.

#### `rerun_py`

* Dataframe queries now use the new string representation everywhere. It's not allowed to pass in components anymore.
* `resolve_component_column_selector` now returns an `Option`.

With tagged components, our data model becomes much more flexible.
We can now have multiple components of the same type living on the same archetype, so we also need a new way to identify columns in a recording.
For this we introduce the following new string-based representation:

```
<entity_path>:[<archetype_name>]:<component>
```

Note that the `archetype_name` section is optional, because components can also be logged as plain fields.

### Blueprints

We have updated our logic so that the `component` field of `blueprint.datatypes.ComponentColumnSelector` follows the same schema.

#### LeRobot dataloader

* Fixed an issue where the LeRobot dataloader logged untagged `Name` components for robot observations and actions, `.rrd` files created before `0.24` may include these untagged entries. To fix this, load the dataset in `0.24.0` and resave your episodes to `.rrd` (`0.24.0` now supports saving all selected recordings).

## Dataframe API: `View.select_static` is deprecated

The dataframe API was originally introduced with the `View.select_static()` variant of `View.select()` used when the view only contains static columns. In such cases, `select()` would yield no row as no data is logged at any index value (by definition of static-only content). Instead, `select_static()` would force the generation of a single, index-less row populated with the static data.

The same behavior can now be achieved by using `Recording.view(index=None, content=...)`.
