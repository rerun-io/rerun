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


## Tagged components

<!-- TODO(grtlr): These are ad-hoc notes from https://github.com/rerun-io/rerun/pull/10082 and need to be cleaned up! -->

#### `re_types_core`

* The obvious `ComponentDescriptor` changes.

#### `re_types`

* Indicator components now become indicator fields. This is only temporary until we get rid of indicators.

#### `re_log_types`

* Removed auto-filling of `rerun.components.` to `ComponentName` in path parsing.

#### `re_sorbet`

* Lookup `ColumnDescriptor` by `ArchetypeFieldName` instead of `ComponentName`.
* Changed `ComponentColumnSelector`.
* Changed `ComponentColumnDescriptor::column_name` to use fully-qualified column names.

#### `rerun_py`

* Dataframe queries now use the new string representation everywhere. It's not allowed to pass in components anymore.
* `resolve_component_column_selector` now returns an `Option`.
