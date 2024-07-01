---
title: Visualizer and Overrides
order: 650
---

This section explains the process by which logged data is used to produce a visualization and how it can be customized via the user interface or code.

*Note*: this area is under heavy development and subject to changes in future releases.

## How are visualizations produced?

[![image.png](https://i.postimg.cc/FRjRzX5g/image.png)](https://postimg.cc/47xZ2M3m)

In the Rerun viewer, visualizations happen within _views_, which are defined by their [_blueprint_](blueprint.md).

The first step for a view to display its content is to determine which entities are involved.
This is determined by the [entity query](../reference/entity-queries.md), which is part of the view blueprint.
The query is run against the data store to generate the list of view entities.

Views rely on visualizers to display each of their entities. For example, [3D views](../reference/types/views/spatial3d_view.md) use the `Points3D` visualizer to display 3D point clouds, and [time series views](../reference/types/views/time_series_view.md) use the `SeriesLine` visualizer to display time series line plots. Which visualizers are available is highly dependent on the specific kind of view. For example, the `SeriesLine` visualizer only exist for time series views—not, e.g., 3D views.

For a given view, visualizers are selected for each of its entities based on their content.
By default, visualizers are selected for entities logged with a corresponding [archetype](../reference/types/archetypes.md).
For example, in a 3D view, an entity logged with the [`PointsD`](../reference/types/archetypes/points3d.md) results in the `Points3D` visualizer being selected by default.
This happens because [archetypes](../reference/types/archetypes.md) include an _indicator component_ to capture the intent of the logging code.
This indicator component in turn triggers the default activation of the associated visualizer.
(We will see that this process can be influenced by both the user interface and the blueprints.)

Then, each selected visualizer determines the values for the components it supports. For example, the `Points3D` visualizer handles, among others, the [`Position3D`](../reference/types/components/position3d.md), [`Radius`](../reference/types/components/radius.md), and [`Color`](../reference/types/components/color.md) components. For each of these (and the others it also supports), the visualizer must determine a value. By default, it will use the value that was logged to the data store, if any. Otherwise, it will use some fallback value that
 depends on the actual type of visualizer and view. (Again, we will see that this can be influenced by the user interface and the blueprint.)

For an illustration, let's consider a simple example with just two [`Boxes2D`](../reference/types/archetypes/boxes2d.md):

snippet: concepts/viscomp-base

Here is how the user interface represents the `Boxes2D` visualizers in the selection panel, when the corresponding entity is selected:

<img width="50%" src="https://i.postimg.cc/L6gKRBt2/image.png" alt="basic exampleE">


All components used by the visualizer are represented, along with their corresponding values as determined by the visualizer. For the [`Color`](../reference/types/components/color.md) component, we can see both the store and fallback values, the former taking precedence over the latter.



## Per-entity component override

[![image.png](https://i.postimg.cc/s2t2cD4R/image.png)](https://postimg.cc/f3fZWsPH)

To customize a visualization, the blueprint may override any component value for any view entity.
This can be achieved either from the user interface or the logging SDK.
When such an override is defined, it takes precedence over any value that might have been logged to the data store.

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-override

The color of `/boxes/1` is overridden to green. Here is how the user interface represents the corresponding visualizer:

<img width="50%" src="https://i.postimg.cc/zBXDktyT/image.png">

The override is listed above the store and fallback value since it has precedence. It can also be edited or removed from the user interface.


## Per-view component default

[![image.png](https://i.postimg.cc/prK2Wg4b/image.png)](https://postimg.cc/tnC0Dm92)

The blueprint may also specify a default value for all components of a given type, should their value not be logged to the store or overridden for a given view entity. This makes it easy to configure visual properties for a potentially large number of entities a view may contain.

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-default

Here, the `/boxes/2` entity is no longer logged with a color value, but a default color is added to the blueprint. Here is how the user interface represents its visualizer:

<img width="50%" src="https://i.postimg.cc/pLbs53Sc/image.png" alt="component override example">

The default color value is displayed above the fallback since it takes precedence. It can also be edited or removed from the user interface.

All component default values are displayed in the selection panel when selecting the corresponding view:

<img src='https://i.postimg.cc/pX8gSQb7/image.png' width="50%" alt='image'/>

Again, it is possible to manually add, edit, and remove component defaults from the user interface.


## Component value resolution order

The previous sections showed that visualizers use a variety of sources to determine the values of the components they are interested in. Here is a summary of the priority order:

1. **Override**: the per-entity override (the highest priority)
2. **Store**: the value that was logged to the data store (e.g., with the `rr.log()` API)
3. **Default**: the default value for this component type
4. **Fallback**: a context-specific fallback value which may depend on the specific visualizer and view type (the lowest priority)

As an illustration, all four values are available for the `/boxes/1` entity of the previous example. Here is how its visualizer is represented in the user interface:

<img src="https://i.postimg.cc/W4K8jK07/image.png" width="50%">


## Visualizer override

So far, we discussed how visualizers determine values for the components they are interested in and how this can be customized. This section instead discusses the process of how visualizers themselves are determined and how to override this process.

⚠️NOTE: the feature covered by this section, including its API, is very likely to change in future releases
(relevant [issue](https://github.com/rerun-io/rerun/issues/6626)).

[![image.png](https://i.postimg.cc/rzXWP2XD/image.png)](https://postimg.cc/SYdJn5K4)

In the previous examples, because [`Boxes2D`](../reference/types/archetypes/boxes2d.md) archetypes were used for logging then entities, `Boxes2D` visualizers were automatically selected. A key factor driving this behavior is the `Boxes2DIndicator` component, which is a data-less marker automatically inserted by the corresponding `Boxes2D` archetype. This is, however, not the only visualizer capable of displaying these entities. The `Point2D` visualizer can also be used, since it only requires [`Position2D`](../reference/types/components/position2d.md) components.

Here is how to force a `Points2D` visualizer for `/boxes/1`, instead of the default `Boxes2D` visualizer:

snippet: concepts/viscomp-visualizer-override

The view now displays a point instead of the box. Here is how the visualizer is displayed in the user interface (note the visualizer of type `Points2D`):

<img src="https://i.postimg.cc/V6k3d2h0/image.png" width="50%">

It is also possible to have _multiple_ visualizers for the same view entity by using an array:

snippet: concepts/viscomp-visualizer-override-multiple

In this case, both a box and a point will be displayed. Adding and removing visualizers is also possible from the user interface.
