---
title: Customize your visualization
order: 100
description: Visualizers, overrides, and per-entity customization in views
---

This page explains how the Viewer turns logged data into a visualization, and how to change that per view, from the UI or from code.
## How a view draws your data

<!-- schematics source: https://excalidraw.com/#json=AZT206K0Tsph5vuZpJHFA,Tc13rWiMnD2ISMWHH7t2QA -->

<picture>
  <img src="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/full.png" alt="Diagram of a view blueprint: entities and component mappings (fed by overrides and defaults) go into visualizers that render the view">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/1200w.png">
</picture>

In the Rerun Viewer, visualizations happen within _views_, which are defined by their [_blueprint_](../inspect-recordings/recording-layout.md).

The first step for a view to display its content is to determine which entities are involved.
This is determined by the [entity path filter](entity-path-filter.md), which is part of the view blueprint.
The query is run against the chunk store to generate the list of view entities.

Views rely on visualizers to display each of their entities.
For example, [3D views](../../reference/types/views/spatial3d_view.md) use the `Points3D` visualizer to display 3D point clouds,
and [time series views](../../reference/types/views/time_series_view.md) use the `SeriesLines` visualizer to display time series line plots.
Which visualizers are available is highly dependent on the specific kind of view.
For example, the `SeriesLines` visualizer only exists for time series views — not, e.g., for 3D views.

For a given view, each entity's components determine which visualizers are available.
For most visualizers, this is decided by Arrow datatype.
The visualizer is available when the entity has a component with the datatype of the visualizer's required input, whatever that component is named.
By default, visualizers are selected for entities logged with a corresponding [archetype](../../reference/types/archetypes.md).
For example, in a 3D view, an entity logged with the [`Points3D`](../../reference/types/archetypes/points3d.md) archetype results in the `Points3D` visualizer being selected by default.
This happens because the components of an [archetype](../../reference/types/archetypes.md) are tagged with the archetype's name.
With a few exceptions, archetypes are directly associated with a single visualizer, but it's also possible to add multiple visualizers of the same type to a given entity via blueprints or the UI.

Then, each selected visualizer determines the values for its inputs. For example, the `Points3D` visualizer reads, among others, `Points3D:positions` ([`Position3D`](../../reference/types/components/position3d.md)), `Points3D:radii` ([`Radius`](../../reference/types/components/radius.md)), and `Points3D:colors` ([`Color`](../../reference/types/components/color.md)).

<!-- It's a feature that we don't need to specify any details for the visualizers here. -->

## Choose visualizers

Sometimes it makes sense to explicitly set the visualizers, to change the way entities are visualized.

Here is how to force a `SeriesPoints` visualizer for `/trig/sin`, in addition to the default `SeriesLines` visualizer:

snippet: tutorials/visualizer-overrides

The view now displays a series of points in addition to connecting the values with lines.
Here is how the visualizers are displayed in the user interface:

<picture>
  <img src="https://static.rerun.io/series_points_visualizer/affbe3fee18bb09057d29f27cf3993ab5a14f061/full.png" alt="SeriesPoints visualizer in the selection panel with scalars, colors, markers set to Circle, names, visible_series and marker_sizes">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_points_visualizer/affbe3fee18bb09057d29f27cf3993ab5a14f061/480w.png">
</picture>

The next section describes how to precisely control what data each visualizer operates on, to fully customize the contents of a view.

<span id="component-mappings"></span>
<span id="selectors-for-nested-data"></span>

For component mappings and nested selectors, see [Remap your data](component-mappings.md).

## Where a visualizer input gets its value

A visualizer normally reads components from the recording.
If a value is missing, it can use a view default; you can also override a value for an entity or [remap an input](component-mappings.md) to another component.
Required inputs, such as point positions, must be available for the visualizer to draw data.

## Set a value for one entity

An override sets a custom value for one entity in one view.
A common way of customizing a visualization is by setting custom values, for example for visualizers that expect a [`Color`](../../reference/types/components/color.md) component.
In the UI this can be done via the visualizer UI, by clicking and modifying the color component, or by selecting "Add custom" from the Source dropdown.

When such a customization is defined, it automatically changes the component's source for this visualizer to point to this new custom value.

The Source dropdown menu allows quick toggling between the different input representations.
By clicking on "Add custom" you can set a custom value:

<picture>
  <img src="https://static.rerun.io/viscomp-add-custom/ac6e0df27139c7be2f446c17981bed74509c0b31/full.png" alt="Source dropdown for the Boxes2D colors component, listing Boxes2D:colors, View default and the highlighted Add custom option">
</picture>

You then can use the color picker to determine a color:

<picture>
  <img src="https://static.rerun.io/viscomp-color-picker/9d7d054c9374a63d9c68e1c89ee840518d614717/full.png" alt="Color picker open on the colors component of a Boxes2D visualizer, set to bright green (R 71, G 255, B 0)">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-color-picker/9d7d054c9374a63d9c68e1c89ee840518d614717/480w.png">
</picture>

Note that any direct edit on any component of the visualizer will always set the source to "Custom".


The following snippet shows how the same customization can be achieved with the blueprint API:

snippet: concepts/viscomp-component-override


## Set a default for a view

You can also force a visualizer to use a specific source kind.
Setting the source to `Default` makes the visualizer ignore any store data and use the view's default instead:

snippet: howto/component_mapping[custom_value]

### View defaults

The viewer picks default component values based on a wide variety of different heuristics, ranging from simple local properties like
an entity's name all the way to things like the size of a view's bounding box or how many plots are within it.

Sometimes, it makes sense to set a _custom_ default value that is applied across all
visualizers of a view, to avoid redundant blueprint definitions.

This can be done through the UI by selecting the view in question and specifying a new default there:

<picture>
  <img src="https://static.rerun.io/components_default_view/285a714b9ae87afa18fd1492a10d3bb62a9c5800/full.png" alt="Component defaults section of a selected view, with a Pinhole image_plane_distance default set to 5.00">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/components_default_view/285a714b9ae87afa18fd1492a10d3bb62a9c5800/480w.png">
</picture>

A custom default value set this way will show up on the respective visualizers:

<picture>
  <img src="https://static.rerun.io/components_default_plane_distance/d279824ff5fe94fee7787550d22df313f46eac2c/full.png" alt="Source dropdown for image_plane_distance with View default selected and Add custom as the other option">
</picture>

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-default

Here, the `/boxes/2` entity is no longer logged with a color value, but a default box color is added to the blueprint. Here is how the user interface represents its visualizer:

<picture>
  <img src="https://static.rerun.io/viscomp-component-default-1/47dc9337eb6aae6b7c82e44409cd83e06689cd38/full.png" alt="Boxes2D visualizer in the selection panel with a blue colors value whose Source is set to View default">
</picture>

And as before, this also shows up in the View's component defaults.

<picture>
  <img src="https://static.rerun.io/viscomp-component-default-2/c6943e2139dc7271075999a71b09596f07aa0776/full.png" alt="Component defaults section of a view listing a blue colors default for Boxes2D">
</picture>


## Override path rules

Keys in `overrides={…}` must be full entity paths from the root.
They do not support `$origin` or wildcards such as `/**`.
