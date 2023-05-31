---
title: Entities and Components
order: 1
---

## Data model
The core of Rerun's data model is inspired by the ideas of the Entity Component System (ECS) architecture pattern. In
short, an ECS is a composition-oriented framework in which Entities represent generic objects while Components describe
data associated with those Entities.

 * Entities are the "things" that your log statements talk about. They are represented by the
   [Entity Path](entity-path.md) string that is the first argument to most of the logging APIs.
 * Components, however, are what contains the data that is associated with those "things". For example, position, color,
   pixel data, etc.
### Logging data
All of the data that you log within rerun is mapped to the concepts of entities and components.
For example, consider the case of logging a point
```python
rr.log_point("my_point", position=[32.7, 45.9], color=[255, 0, 0])
```
This log statement is recording data about the Entity "my_point". The data will ultimately be stored in two components.
In this case `point2d` and `colorrgba`.  Behind the scenes, this function is simply making records in the data store
that these component values are associated with the "my_point" entity.

### Primitives
Later, the Space View for spatial types queries the data store for all of the entities that have a `point2d` component.
In this case it would find the "my_point" entity. This query additionally returns the `colorrgba` component because that
component is associated with the same entity.

We call this pre-defined collection of components a _Primitive_.
Primitives do not have any significance to the data model itself, but are important for the Viewer to understand
how data should be displayed.

The assorted logging APIs all simply set different combinations of components on some specified entity, and the
corresponding space views look for entities with these components in the data store.  
For more information on the different primitives and how they relate to components see the
[Primitives reference](../reference/primitives.md).

### Extension Components
Your entity could have any number of other components as well. This isn't a problem. Any components that
aren't relevant to the scene that the space view is drawing are safely ignored. In fact, Rerun even allows you to log your
own components.

In Python this is done via [log_extension_components](https://ref.rerun.io/docs/python/latest/common/extension_components/#rerun.log_extension_components)
, whereas in Rust you implement the [Component](https://docs.rs/rerun/latest/rerun/trait.Component.html) trait.

code-example: extension-components



### Empty Entities
An Entity, without Components, is nothing more than an identity (represented by its Entity
Path). It contains no data, and has no type. When you log a piece of data, all that you are doing is setting the values
of one or more *Components* associated with that *Entity*. 

## ECS Systems
In most ECS architectures, there is a third concept we haven't touched on: Systems are processes which operate on the
Entities based on the Components they possess.

Rerun does not currently have formalized Systems, although the patterns employed by the Rerun Space Views are very much
"System like" in their operation. Proper Systems may be a feature investigated in the future
([#1155](https://github.com/rerun-io/rerun/issues/1155)).
