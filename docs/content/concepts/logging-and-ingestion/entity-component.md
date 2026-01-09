---
title: Entities and Components
order: 200
---

## Data model

The core of Rerun's data model is inspired by the ideas of the [Entity Component System (ECS)](https://en.wikipedia.org/wiki/Entity_component_system) architecture pattern. In
short, an ECS is a composition-oriented framework in which *entities* represent generic objects while *components* describe
data associated with those entities.

 * *Entities* are the "things" that you log with the [`rr.log()`](https://ref.rerun.io/docs/python/stable/common/logging_functions/#rerun.log)function. They are represented by the
   [*entity path*](entity-path.md) string which is passed as first argument.
 * *Components*, however, are what contains the data that is associated with those "things". For example, position, color,
   pixel data, etc.

Entities are like folders, and components are like files.

Additionally, the Rerun SDKs expose two additional concepts:
 * *Archetypes* are coherent set of components corresponding to primitive such as 2D points or 3D boxes. In the Rerun SDKs, archetypes take the form of builder objects that assist with the creation of such component sets. They are meant as high-level, convenience helpers that can be bypassed entirely if/when required by advanced use-cases.
 * *Datatypes* are regular data structures that components occasionally rely on when fundamental data types (`float`, `uint32`, etc.) are not sufficient.

### Logging and viewing data

All the data that you log within Rerun is mapped to the concepts of entities and components.
For example, consider the case of logging a point:

```python
rr.log("my_point", rr.Points2D([32.7, 45.9], colors=[255, 0, 0]))
```

This statement uses the [`rr.Points2D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points2D) archetype.
Internally, this archetype builds a set of, in this case, two components:
* `Points2D:positions` of type [`Position2D`](../../reference/types/components/position2d.md)
* `Points2D:colors` of type [`Color`](../../reference/types/components/color.md).
Then, the `rr.log()` function records these two components and associate them with the `"my_point"` entity.

Later, the View for spatial types queries the data store for all the entities that have a `Points2D:positions` component.
In this case it would find the "my_point" entity. This query additionally returns the `Points2D:colors` component because that
component is associated with the same entity. These two components are recognized as corresponding to the `Points2D` archetype (via metadata attached to the components), which informs the Viewer on how to display the corresponding entity.

See the [Types](../../reference/types.md) reference for a list of [archetypes](../../reference/types/archetypes.md),
[components](../../reference/types/components.md), and [datatypes](../../reference/types/datatypes.md).

### Adding custom data

Although both the SDKs' archetype objects and the view are based on the same archetype definition (and are actually implemented using code that is automatically generated based on that definition), they both operate on arbitrary collection
of components. Neither the SDKs nor the Viewer enforce or require that an entity should contain a *specific* set of component.
The Rerun Viewer will display any data in a generic form, but its views will only work on sets of components it can
make sense of.

Your entity could have any number of additional components as well. This isn't a problem. Any components that
aren't relevant to the scene that the view is drawing are safely ignored. Also, Rerun even allows you to log your
own set of components, bypassing archetypes altogether.

In Python, the [rr.AnyValues](https://ref.rerun.io/docs/python/stable/common/custom_data/#rerun.AnyValues) helper object can be used to add custom component(s) to an archetype:

snippet: tutorials/extra_values

It can also be used log an entirely custom set of components:

snippet: tutorials/any_values

For more complex use-cases, custom objects implementing the `rr.AsComponents` protocol can be used. For Rust, the `rerun::AsComponents` trait must be implemented:

snippet: tutorials/custom_data

### Empty entities

An entity without components is nothing more than an identity (represented by its entity
path). It contains no data, and has no type. When you log a piece of data, all that you are doing is setting the values
of one or more components associated with that entity.

## ECS systems

There is a third concept we haven't touched on: *systems* are processes which operate on the entities based on the components they possess.
Rerun is still settling on the exact form of formalized systems and outside of Rust Viewer code it is not yet possible to write your own systems. However, views work under the hood using a variety of systems. For more information see the [Extend the Viewer in Rust](../../howto/extend/extend-ui.md) section.
