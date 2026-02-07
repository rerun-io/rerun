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

## Micro-batcher default flushing duration increased from 8ms to 200ms for memory & file recording streams

`RERUN_FLUSH_TICK_SECS` previously always defaulted to 8ms when left unspecified.
This now only applies to recording streams that use a GRPC connection, all others default to 200ms.

You can learn more about micro-batching in our [dedicated documentation page](../sdk/micro-batching.md).

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
This severely effected the design and flexibility of our archetypes.
The underlying reason for this was that the our internal datastructures used the component's type names (previously also referred to as `ComponentName`) to index into the data.
This release changes how components are identified within the viewer and within our APIs:
Instead of specifying the component's type name, components are now referenced by a new syntax that consists of the (short) archetype name + the archetype field name separated by a colon (`:`).

As an example, `Points3D:positions` refers to the `positions` component in the `rerun.archetypes.Points3D` archetype.
(Previously, it would only be identified by its component type, i.e. `rerun.components.Position3D`.)
For custom data, such as `AnyValues`, it is possible to omit the archetype part of this syntax and only specify the field name.

To `View.select` columns in dataframe queries, we additionally need to specify the entity that a component belongs to.
Starting with this release, this is achieved by adding the `entity_path` as a prefix to the component identifier, for example: `/helix/structure/left:Points3D:positions`.
In general, the syntax for uniquely identifying a component in recording then becomes:

```
<entity_path>:[<archetype>:]<field>
```

### Custom data

Internally we use, what we call a `ComponentDescriptor` to describe the structure and semantics of a component.
Conceptually, it looks like the following:

```rs
struct ComponentDescriptor {
  archetype:      Option<String>, // e.g. `rerun.archetypes.Points3D
  component:      String,         // e.g. `Points3D:positions`
  component_type: Option<String>, // e.g. `rerun.components.Position3D
}
```

Custom data can still use simple field names such as `confidences`, but it is advised to supply a full `ComponentDescriptor`, if possible.
For this we also provide a new `AnyValues.with_field` method.

### Changes

When logging data to Rerun using the builtin archetypes no changes to user code should be required.
There is also migration code in place so that you can open `.rrd` files that were created with `v0.23`.
Recordings from `v0.22` can also be loaded, but need to be migrated using the migration tool from `v0.23` first.

These changes are reflected in various parts of the Rerun viewer:

* It is now possible to log arbitrary overlapping archetypes on a single entity path. It is also now possible to re-use the same component type for different fields of the same archetype.
* The selection panel UI comes with a revamped display of archetypes that uses the new syntax to show the `ComponentDescriptor` for each component.
* The new `:`-based syntax needs to be used when referring to components in the dataframe API and in the dataframe view.
* Changed the interpretation of `blueprint.datatypes.ComponentColumnSelector` to use the new component identifier.
* Indicator components have been removed entirely. The viewer now instead decides which views & visualizers to activate based on archetype information of components.

#### Blueprint component defaults

Blueprint component defaults were previously applied to component _types_.
They are now instead, applies to archetype fields, i.e. what is now just called _component_ (e.g. `GraphNodes:positions`).

In practice this means that component defaults are now limited to a single archetype, making them a lot more useful!

<picture>
  <img src="https://static.rerun.io/visualizer-default-context-menu/9622eae67d9bb17e428fda7242b45b8029639a99/full.png" alt="">
</picture>

### Limitations & breaking changes

* In some cases, it is not possible to migrate previous blueprints, _but only if they were saved from the viewer via the UI_.
* Currently, only Rerun-builtin components are picked up by the visualizers and therefore shown in the views (except for the dataframe view which shows all components).
* In `v0.23`, the LeRobot dataloader logged incomplete `ComponentDescriptors` for robot observations and actions. To fix this, load the dataset in `v0.24` and resave your episodes to `.rrd` (`v0.24` now supports saving all selected recordings).
* Overriding visualizers to reinterpret data (e.g. show a point-cloud for mesh vertices) is no longer possible, since visualizers now match for <archetype>:<field> instead of component type name. This will be addressed in the future with blueprint-driven overrides that will allow to remap data to arbitrary archetypes.
* `VisualizerOverrides` are now limited to time series views, and _stop to be supported for general views_, such as the spatial views.
* The `markers` component on `SeriesPoints` is now marked as _required_, to avoid accidentally logging an archetype without any associated data. In Python, when no component is supplied we automatically set the `markers` shape to `Circle` to avoid breaking user code.

## Dataframe API: `View.select_static` is deprecated

The dataframe API was originally introduced with the `View.select_static()` variant of `View.select()` used when the view only contains static columns. In such cases, `select()` would yield no row as no data is logged at any index value (by definition of static-only content). Instead, `select_static()` would force the generation of a single, index-less row populated with the static data.

The same behavior can now be achieved by using `Recording.view(index=None, content=...)`.
