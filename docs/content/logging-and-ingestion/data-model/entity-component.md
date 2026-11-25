---
title: Entities and Components
order: 200
description: How entities and components form Rerun's ECS-inspired data model
---

## Data model

The core of Rerun's data model is inspired by the ideas of the [Entity Component System (ECS)](https://en.wikipedia.org/wiki/Entity_component_system) architecture pattern. In
short, an ECS is a composition-oriented framework in which *entities* represent generic objects while *components* describe
data associated with those entities.

 * *Entities* are the "things" that you log with the [`rr.log()`](https://ref.rerun.io/docs/python/stable/rerun/#rerun.log) function. They are represented by the
   [*entity path*](entity-path.md) string which is passed as first argument.
 * *Components*, however, are what contains the data that is associated with those "things". For example, position, color,
   pixel data, etc.

Entities are like folders, and components are like files.

Additionally, Rerun has two more concepts:
 * *Archetypes* are named sets of semantically related components. `Points2D`, for example, is positions, colors, and radii. In the Rerun SDKs, archetypes take the form of builder objects that create such a set for you. The archetype name is stored with each component, and the Viewer uses it to choose how to show the data by default. See [Archetypes](archetypes.md).
 * *Encodings* are regular data structures that components occasionally rely on when fundamental data types (`float`, `uint32`, etc.) are not sufficient.

### Logging and viewing data

All the data that you log within Rerun is mapped to the concepts of entities and components.
For example, consider the case of logging a point:

```python
rr.log("my_point", rr.Points2D([32.7, 45.9], colors=[255, 0, 0]))
```

This statement uses the [`rr.Points2D`](https://ref.rerun.io/docs/python/stable/archetypes/#rerun.archetypes.Points2D) archetype.
Internally, this archetype builds a set of, in this case, two components:
* `Points2D:positions` of type [`Position2D`](../../reference/types/components/position2d.md)
* `Points2D:colors` of type [`Color`](../../reference/types/components/color.md).
Then, the `rr.log()` function records these two components and associates them with the `"my_point"` entity.

Later, a 2D view looks for the entities it can draw.
It finds the "my_point" entity, because `Points2D:positions` has the datatype that the `Points2D` visualizer needs.
Both components are tagged with the `Points2D` archetype (via metadata attached to the components), so the Viewer selects the `Points2D` visualizer by default.
That visualizer then reads `Points2D:colors` from the same entity.

See the [Types](../../reference/types.md) reference for a list of [archetypes](../../reference/types/archetypes.md),
[components](../../reference/types/components.md), and [encodings](../../reference/types/encodings.md).

### Adding custom data

Although both the SDKs' archetype objects and the view are based on the same archetype definition, they both operate on arbitrary collections
of components. Neither the SDKs nor the Viewer enforce or require that an entity should contain a *specific* set of components.
The Rerun Viewer will display any data in a generic form, but its views will only work on sets of components it can
make sense of.

Your entity could have any number of additional components as well. This isn't a problem. Any components that
aren't relevant to the scene that the view is drawing are safely ignored. Also, Rerun even allows you to log your
own set of components, bypassing archetypes altogether.

In Python, the [rr.AnyValues](https://ref.rerun.io/docs/python/stable/rerun/#rerun.AnyValues) helper object can be used to log custom component(s) to the same entity as an archetype:

snippet: tutorials/extra_values

It can also be used to log an entirely custom set of components:

snippet: tutorials/any_values

For more complex use-cases, custom objects implementing the `rr.AsComponents` protocol can be used. For Rust, the `rerun::AsComponents` trait must be implemented:

snippet: tutorials/custom_data

### Empty entities

An entity without components is nothing more than an identity (represented by its entity
path). It contains no data, and has no type. When you log a piece of data, all that you are doing is setting the values
of one or more components associated with that entity.

## ECS systems

There is a third concept we haven't touched on: *systems* are processes which operate on the entities based on the components they possess.
You can only write your own systems in Rust Viewer code.
Views use a variety of systems under the hood.
For more information see the [Extend the Viewer in Rust](../../viewer/extend/extend-ui.md) section.
