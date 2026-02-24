---
title: Customize views
order: 200
---

This section explains the process by which logged data is used to produce a visualization and how it can be customized via the user interface or code.

## How are visualizations produced?

<!-- schematics source: https://excalidraw.com/#json=AZT206K0Tsph5vuZpJHFA,Tc13rWiMnD2ISMWHH7t2QA -->

<picture>
  <img src="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/customize-view-diagram/65d8c5a579c74606d147dfa693a09da9fa734411/1200w.png">
</picture>

In the Rerun Viewer, visualizations happen within _views_, which are defined by their [_blueprint_](blueprints.md).

The first step for a view to display its content is to determine which entities are involved.
This is determined by the [entity query](entity-queries.md), which is part of the view blueprint.
The query is run against the data store to generate the list of view entities.

Views rely on visualizers to display each of their entities.
For example, [3D views](../../reference/types/views/spatial3d_view.md) use the `Points3D` visualizer to display 3D point clouds,
and [time series views](../../reference/types/views/time_series_view.md) use the `SeriesLines` visualizer to display time series line plots.
Which visualizers are available is highly dependent on the specific kind of view.
For example, the `SeriesLines` visualizer only exists for time series views—not, e.g., for 3D views.

For a given view, each entity's components determine which visualizers are available.
By default, visualizers are selected for entities logged with a corresponding [archetype](../../reference/types/archetypes.md).
For example, in a 3D view, an entity logged with the [`Points3D`](../../reference/types/archetypes/points3d.md) archetype results in the `Points3D` visualizer being selected by default.
This happens because the components of an [archetype](../../reference/types/archetypes.md) are tagged with the archetype's name.
With a few exceptions, archetypes are directly associated with a single visualizer, but it's also possible to add multiple visualizers of the same type to a given entity via blueprints or the UI.

Then, each selected visualizer determines the values for the components it supports. For example, the `Points3D` visualizer handles, among others, the [`Position3D`](../../reference/types/components/position3d.md), [`Radius`](../../reference/types/components/radius.md), and [`Color`](../../reference/types/components/color.md) components.

<!-- It's a feature that we don't need to specify any details for the visualizers here. -->

Sometimes it makes sense to explicitly set the visualizers, to change the way entities are visualized.

Here is how to force a `SeriesPoints` visualizer for `/trig/sin`, in addition to the default `SeriesLines` visualizer:

snippet: tutorials/visualizer-overrides.py

The view now displays a series of points in addition to connecting the values with lines.
Here is how the visualizers are displayed in the user interface:

<picture>
  <img src="https://static.rerun.io/series_points_visualizer/affbe3fee18bb09057d29f27cf3993ab5a14f061/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/series_points_visualizer/affbe3fee18bb09057d29f27cf3993ab5a14f061/480w.png">
</picture>

The next section describes how to precisely control what data each visualizer operates on, to fully customize the contents of a view.

## Component mappings

Each visualizer takes various components as input. Values are automatically sourced from the data store. When no matching data exists (except for required components like point cloud positions or plot scalars), the Viewer generates sensible default values.
The exact way this is done depends on the type of View, but may be influenced by a variety of circumstances.

Component mappings let you customize this behavior, for example to:

* Control what data is picked from the store - this allows you to visualize arbitrary data, _even when it was not logged with Rerun-semantics_.
* Specify the styling of a visualization as part of your blueprint

Component mappings can be modified via the Viewer UI by navigating to a visualizer and expanding the component of interest.

### Custom values

A common way of customizing a visualization is by setting custom values, for example for visualizers that expect a [`Color`](../../reference/types/components/color.md) component.
In the UI this can be done via the visualizer UI, by clicking and modifying the color component, or by selecting "Add custom…" from the Source dropdown.

When such a customization is defined, it automatically changes the component's source for this visualizer to point to this new custom value.

The Source dropdown menu allows quick toggling between the different input representations.
By clicking on "Add custom…" you can create a new custom component override:

<picture>
  <img src="https://static.rerun.io/viscomp-add-custom/ac6e0df27139c7be2f446c17981bed74509c0b31/full.png" alt="">
</picture>

You then can use the color picker to determine a color:

<picture>
  <img src="https://static.rerun.io/viscomp-color-picker/9d7d054c9374a63d9c68e1c89ee840518d614717/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-color-picker/9d7d054c9374a63d9c68e1c89ee840518d614717/480w.png">
</picture>

Note that any direct edit on any component of the visualizer will always set the source to "Custom".


The following snippet shows how the same customization can be achieved with the blueprint API:

snippet: concepts/viscomp-component-override


### Remapping of components

<!-- TODO(andreas): Should we start lighter and first introduce the concept without arbitrary semantics? -->

A powerful mechanism that is built into visualizers is the option to source components from data that was logged on the same entity but might have arbitrary semantics.

Within a view, a visualizer can pick up any component that has the same datatype as the builtin type that it expects.
For example, the `SeriesLines` and `SeriesPoints` visualizers can pick up any numerical data for their [`Scalar`](../../reference/types/components/scalar.md) component.
The same holds for String-like components that can be selected for [`Name`](../../reference/types/components/name.md).

Such data often comes from MCAP data that has user-defined message types, or from components that were flexibly logged via [`AnyValues`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.AnyValues) or [`DynamicArchetype`](https://ref.rerun.io/docs/python/main/common/custom_data/#rerun.DynamicArchetype).
The Viewer can even look for data with compatible datatypes in nested fields of Arrow [`StructArrays`](https://docs.rs/arrow/latest/arrow/array/struct.StructArray.html).

Suitable components show up in the source dropdown:

<picture>
  <img src="https://static.rerun.io/source_dropdown_scalars/9fb672d6984475010dfb58df485281c327f80368/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/source_dropdown_scalars/9fb672d6984475010dfb58df485281c327f80368/480w.png">
</picture>


<!-- TODO(#12661): adjust docs once ticket is closed -->
> #12661: Currently, only the time series view allows remapping of required components (scalars). All other visualizers require matching Rerun semantics (correct archetype & type metadata) for their required fields.

As always, component mappings can be set via the blueprint APIs:

snippet: howto/component_mapping[source_mapping]


<!-- TODO(grtlr): We probably should create a dedicated selector page in the docs once they have matured a bit. -->
To select nested fields in `StructArrays`, Rerun uses so-called selectors, which are filters that are inspired by [`jq`](https://jqlang.org/), a tool for processing JSON data.

## Per-view component default

The viewer picks default component values based on a wide variety of different heuristics, ranging from simple local properties like
an entity's name all the way to things like the size of a view's bounding box or how many plots are within it.

Sometimes, it makes sense to set a _custom_ default value that is applied across all
visualizers of a view, to avoid redundant blueprint definitions.

This can be done through the UI by selecting the view in question and specifying a new default there:

<picture>
  <img src="https://static.rerun.io/components_default_view/285a714b9ae87afa18fd1492a10d3bb62a9c5800/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/components_default_view/285a714b9ae87afa18fd1492a10d3bb62a9c5800/480w.png">
</picture>

A custom default value set this way will show up on the respective visualizers:

<picture>
  <img src="https://static.rerun.io/components_default_plane_distance/d279824ff5fe94fee7787550d22df313f46eac2c/full.png" alt="">
</picture>

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-default

Here, the `/boxes/2` entity is no longer logged with a color value, but a default box color is added to the blueprint. Here is how the user interface represents its visualizer:

<picture>
  <img src="https://static.rerun.io/viscomp-component-default-1/47dc9337eb6aae6b7c82e44409cd83e06689cd38/full.png" alt="">
</picture>

And as before, this also shows up in the View's component defaults.

<picture>
  <img src="https://static.rerun.io/viscomp-component-default-2/c6943e2139dc7271075999a71b09596f07aa0776/full.png" alt="">
</picture>
