---
title: Entities and Components
order: 1
---

## Data model

The core of Rerun's data model is inspired by the ideas of the [Entity Component System (ECS)](https://en.wikipedia.org/wiki/Entity_component_system) architecture pattern. In
short, an ECS is a composition-oriented framework in which *entities* represent generic objects while *components* describe
data associated with those entities.

 * *Entities* are the "things" that you log with the [`rr.log()`](https://ref.rerun.io/docs/python/HEAD/common/logging/#rerun.log)function. They are represented by the
   [*entity path*](entity-path.md) string which is passed as first argument.
 * *Components*, however, are what contains the data that is associated with those "things". For example, position, color,
   pixel data, etc.
 
Additionally, the Rerun SDKs expose two additional concepts:
 * *Archetypes* are coherent set of components corresponding to primitive such as 2D points or 3d boxes. In the Rerun SDKs, archetypes take the form of builder objects that assist with the creation of such component sets. They are meant as high-level, convenience helpers that can be bypassed entirely if/when required by advanced use-cases.
 * *Datatypes* are regular data structures that components occasionally rely on when fundamental data types (`float`, `uint32`, etc.) are not sufficient.

### Logging and viewing data

All the data that you log within rerun is mapped to the concepts of entities and components.
For example, consider the case of logging a point:

```python
rr.log("my_point", rr.Points2D([32.7, 45.9], color=[255, 0, 0]))
```

This statement uses the [`rr.Points2D`](https://ref.rerun.io/docs/python/HEAD/common/spatial_primitives/#rerun.Points2D) archetype.
Internally, this archetype builds a set of, in this case, two components: [`Position2D`](../reference/data_types/components/position2d.md) and [`Color`](../reference/data_types/components/color.md). Then, the
`rr.log()` function records these two components and associate them with the `"my_point"` entity.

Later, the Space View for spatial types queries the data store for all the entities that have a `Position2D` component.
In this case it would find the "my_point" entity. This query additionally returns the `Color` component because that
component is associated with the same entity. These two components are recognized as corresponding to the `Points2D` archetype, which informs the viewer on how to display the corresponding entity.

See the [Data Types](../reference/data_types.md) reference for a list of archetypes, components, and datatypes.

### Extension Components

Although both the SDKs' archetype objects and the space view are based on the same archetype definition (and are actually implemented using code that is automatically generated based that definition), they both operate on arbitrary collection
of components. Neither the SDKs nor the viewer enforce or require that an entity should contain a *specific* set of component.
The Rerun viewer will display any data in a generic form, but its space views will only work on sets of components it can
make sense of.

Your entity could have any number of additional components as well. This isn't a problem. Any components that
aren't relevant to the scene that the space view is drawing are safely ignored. Also, Rerun even allows you to log your
own set of components, bypassing archetypes altogether.

In Python this is done via [log_extension_components](https://ref.rerun.io/docs/python/latest/common/extension_components/#rerun.log_extension_components), whereas in Rust you implement the [`Component`](https://docs.rs/rerun/latest/rerun/experimental/trait.Component.html) trait.

TODO: FIXME!!!

code-example: extension-components

### Empty Entities

An entity without components is nothing more than an identity (represented by its entity
path). It contains no data, and has no type. When you log a piece of data, all that you are doing is setting the values
of one or more components associated with that entity.

## ECS Systems

In most ECS architectures, there is a third concept we haven't touched on: *systems* are processes which operate on the
entities based on the components they possess. Rerun does not currently have formalized systems, although the patterns employed by the space views are very much "System like" in their operation. Proper Systems may be a feature investigated in the future
([#1155](https://github.com/rerun-io/rerun/issues/1155)).