---
title: Visualizers and Overrides
order: 650
---

This section explains the process by which logged data is used to produce a visualization and how it can be customized via the user interface or code.

*Note*: this area is under heavy development and subject to changes in future releases.

## How are visualizations produced?

<!-- schematics source: https://excalidraw.com/#json=8G274_acK-zYc7Cq2ONf0,GaIabh3FBulcjNx9ZqJrXg -->

<picture>
  <img src="https://static.rerun.io/viscomp-base/02d6fe87db0d33b6e9e4dc2d647b3c473e6ce50b/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-base/02d6fe87db0d33b6e9e4dc2d647b3c473e6ce50b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viscomp-base/02d6fe87db0d33b6e9e4dc2d647b3c473e6ce50b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viscomp-base/02d6fe87db0d33b6e9e4dc2d647b3c473e6ce50b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viscomp-base/02d6fe87db0d33b6e9e4dc2d647b3c473e6ce50b/1200w.png">
</picture>

In the Rerun viewer, visualizations happen within _views_, which are defined by their [_blueprint_](blueprint.md).

The first step for a view to display its content is to determine which entities are involved.
This is determined by the [entity query](../reference/entity-queries.md), which is part of the view blueprint.
The query is run against the data store to generate the list of view entities.

Views rely on visualizers to display each of their entities.
For example, [3D views](../reference/types/views/spatial3d_view.md) use the `Points3D` visualizer to display 3D point clouds,
and [time series views](../reference/types/views/time_series_view.md) use the `SeriesLines` visualizer to display time series line plots.
Which visualizers are available is highly dependent on the specific kind of view.
For example, the `SeriesLines` visualizer only exist for time series views—not, e.g., for 3D views.

For a given view, each entity's components determine which visualizers are available.
By default, visualizers are selected for entities logged with a corresponding [archetype](../reference/types/archetypes.md).
For example, in a 3D view, an entity logged with the [`Points3D`](../reference/types/archetypes/points3d.md) archetype results in the `Points3D` visualizer being selected by default.
This happens because [archetypes](../reference/types/archetypes.md) include an _indicator component_ to capture the intent of the logging code.
This indicator component in turn triggers the default activation of the associated visualizer.
(We will see that this process can be influenced by both the user interface and the blueprints.)

Then, each selected visualizer determines the values for the components it supports. For example, the `Points3D` visualizer handles, among others, the [`Position3D`](../reference/types/components/position3d.md), [`Radius`](../reference/types/components/radius.md), and [`Color`](../reference/types/components/color.md) components. For each of these (and the others it also supports), the visualizer must determine a value. By default, it will use the value that was logged to the data store, if any. Otherwise, it will use some fallback value that
 depends on the actual type of visualizer and view.

For an illustration, let's consider a simple example with just two [`Boxes2D`](../reference/types/archetypes/boxes2d.md):

snippet: concepts/viscomp-base

Here is how the user interface represents the `Boxes2D` visualizers in the selection panel, when the corresponding entity is selected:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-base-screenshot/80f168067b49d2a40aed41b0f3512117314c6a9d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-base-screenshot/80f168067b49d2a40aed41b0f3512117314c6a9d/480w.png">
</picture>


All components used by the visualizer are represented, along with their corresponding values as determined by the visualizer. For the [`Color`](../reference/types/components/color.md) component, we can see both the store and fallback values, the former taking precedence over the latter.



## Per-entity component override

<picture>
  <img src="https://static.rerun.io/viscomp-component-override/aebe94bb431e28d49acd5e5cedc6bfe4905ff1c5/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-override/aebe94bb431e28d49acd5e5cedc6bfe4905ff1c5/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viscomp-component-override/aebe94bb431e28d49acd5e5cedc6bfe4905ff1c5/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viscomp-component-override/aebe94bb431e28d49acd5e5cedc6bfe4905ff1c5/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viscomp-component-override/aebe94bb431e28d49acd5e5cedc6bfe4905ff1c5/1200w.png">
</picture>

To customize a visualization, the blueprint may override any component value for any view entity.
This can be achieved either from the user interface or the logging SDK.
When such an override is defined, it takes precedence over any value that might have been logged to the data store.

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-override

The color of `/boxes/1` is overridden to green. Here is how the user interface represents the corresponding visualizer:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-component-override-screenshot/cfd1498e18279734a2d494778bf2e6b603b3b44e/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-override-screenshot/cfd1498e18279734a2d494778bf2e6b603b3b44e/480w.png">
</picture>

The override is listed above the store and fallback value since it has precedence. It can also be edited or removed from the user interface.


## Per-view component default

<picture>
  <img src="https://static.rerun.io/viscomp-component-default/8473f99cc1cad8f6d15a16019c2c0d18edd77220/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-default/8473f99cc1cad8f6d15a16019c2c0d18edd77220/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viscomp-component-default/8473f99cc1cad8f6d15a16019c2c0d18edd77220/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viscomp-component-default/8473f99cc1cad8f6d15a16019c2c0d18edd77220/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viscomp-component-default/8473f99cc1cad8f6d15a16019c2c0d18edd77220/1200w.png">
</picture>

The blueprint may also specify a default value for all components of a given type, should their value not be logged to the store or overridden for a given view entity. This makes it easy to configure visual properties for a potentially large number of entities.

This is how it is achieved with the blueprint API:

snippet: concepts/viscomp-component-default

Here, the `/boxes/2` entity is no longer logged with a color value, but a default color is added to the blueprint. Here is how the user interface represents its visualizer:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-component-default-screenshot-1/5966fde4bdddd5e8ef07c6c0a0576b4a487b644e/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-default-screenshot-1/5966fde4bdddd5e8ef07c6c0a0576b4a487b644e/480w.png">
</picture>

The default color value is displayed above the fallback since it takes precedence. It can also be edited or removed from the user interface.

All component default values are displayed in the selection panel when selecting the corresponding view:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-component-default-screenshot-2/2db4ca28ff94c1de58a750855828024aa4043576/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-default-screenshot-2/2db4ca28ff94c1de58a750855828024aa4043576/480w.png">
</picture>

Again, it is possible to manually add, edit, and remove component defaults from the user interface.


## Component value resolution order

The previous sections showed that visualizers use a variety of sources to determine the values of the components they are interested in. Here is a summary of the priority order:

1. **Override**: the per-entity override (the highest priority)
2. **Store**: the value that was logged to the data store (e.g., with the `rr.log()` API)
3. **Default**: the default value for this component type
4. **Fallback**: a context-specific fallback value which may depend on the specific visualizer and view type (the lowest priority)

As an illustration, all four values are available for the `/boxes/1` entity of the previous example. Here is how its visualizer is represented in the user interface:

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-component-value-resolution-screenshot/4ecd7b4be069f5d77d0ea541dc1f47d66a868e2d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-component-value-resolution-screenshot/4ecd7b4be069f5d77d0ea541dc1f47d66a868e2d/480w.png">
</picture>


## Visualizer override

So far, we discussed how visualizers determine values for the components they are interested in and how this can be customized. This section instead discusses the process of how visualizers themselves are determined and how to override this process.

⚠️NOTE: the feature covered by this section, including its API, is very likely to change in future releases
(relevant [issue](https://github.com/rerun-io/rerun/issues/6626)).

<picture>
  <img src="https://static.rerun.io/viscomp-full/945b98084d12be14a5258f2ba00786cb6ec7d19a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-full/945b98084d12be14a5258f2ba00786cb6ec7d19a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viscomp-full/945b98084d12be14a5258f2ba00786cb6ec7d19a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viscomp-full/945b98084d12be14a5258f2ba00786cb6ec7d19a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viscomp-full/945b98084d12be14a5258f2ba00786cb6ec7d19a/1200w.png">
</picture>


In the previous examples, because [`Boxes2D`](../reference/types/archetypes/boxes2d.md) archetypes were used for logging then entities, `Boxes2D` visualizers were automatically selected. A key factor driving this behavior is the `Boxes2DIndicator` component, which is a data-less marker automatically inserted by the corresponding `Boxes2D` archetype. This is, however, not the only visualizer capable of displaying these entities. The `Point2D` visualizer can also be used, since it only requires [`Position2D`](../reference/types/components/position2d.md) components.

Here is how to force a `Points2D` visualizer for `/boxes/1`, instead of the default `Boxes2D` visualizer:

snippet: concepts/viscomp-visualizer-override

The view now displays a point instead of the box. Here is how the visualizer is displayed in the user interface (note the visualizer of type `Points2D`):

<picture style="zoom: 0.5">
  <img src="https://static.rerun.io/viscomp-visualizer-override-screenshot/fdb3e9ae8afa3c2a6bf1fc8836e0365aef393970/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viscomp-visualizer-override-screenshot/fdb3e9ae8afa3c2a6bf1fc8836e0365aef393970/480w.png">
</picture>

It is also possible to have _multiple_ visualizers for the same view entity by using an array:

snippet: concepts/viscomp-visualizer-override-multiple

In this case, both a box and a point will be displayed. Adding and removing visualizers is also possible from the user interface.
