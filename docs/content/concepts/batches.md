---
title: Batch Data
order: 5
---

Rerun has built-in support for batch data. Whenever you have a collection of things that all have the same type, rather
than logging each element individually, you can log the entire collection together as a single "Batch". This provides
significant benefits in terms of storage and compute.

Some examples of batched data include points in a point cloud, bounding boxes for detected objects, tracked keypoints
in a skeleton, or a collection of line strips.

In the Python APIs, the majority of archetypes are named with the plural form, for example [`rr.Points3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points3D). They accept both single elements (internally treated as an N=1 batch) or arrays corresponding to the batches.

## Terminology

- An *entity* is a collection of *components* (see [Entities and Components](entity-component.md)).
- When an entity is batched, it's components individual elements are called *instances*.
- Each instance is identified within the entity by its *instance key*.
- When every instance within an entity has the same value for a component, that component is called a *splat*. This
  is a common pattern and has dedicated support for it (see the [Splats](#splats) section below).
  For instance, you can set all the colors of a point cloud to the same color by passing a single color value to the
 `color` parameter.
- During queries, a batch always has a *primary* component. The primary component is what determines
  how many instances exist in the batch.

## Restrictions

When using batched entities there are a few restrictions:
 - Because there is a one-to-one mapping between batches and entities:
    - If data needs to span multiple entity paths, it needs to be split up into separate batches.
    - If data needs to be split into multiple batches, each must be logged to a different path.
 - Whenever you log a batched entity, for any component that is updated, you must provide values for
   every instance.
    - It is not possible to only update a subset of the instances in the batch.

## Batch join rules

Rerun lets you choose which components in an entity you want to log at any point in time. If you don't log to a
component, then in general it is not updated. For example, if you log a point cloud with positions and colors and then
later log just new positions, when the viewer displays that point cloud it will still look up the *last* colors that
were logged (we refer to this as the *latest at* semantics).

This can be quite convenient since updating different components at different times puts far fewer restrictions on the
organization of your code. It even means if a component on an entity is static, you only need to log it once.

However, if both a batch of colors and a batch of positions have been logged at two different points in time, we need a way
to know which point receives which color. This is what Rerun uses the instance keys for. When a component batch is
logged it is always assigned a set of instance keys. By default, this key is based on the sequential index within the
logged array. When querying a batched component, the component-values are joined together based on these keys.
Logically, this happens as a *left-join* using the primary component for the entity. For example, if you log 3
points and then later log 5 colors, you will still only see 3 points in the viewer.

In the future you will be able to specify the instance keys manually while logging ([#1309](https://github.com/rerun-io/rerun/issues/1309)).

## Splats

As mentioned, Rerun has special handling for splats within batches. This is what happens when you mix arrays and
single values in an API call. The non-array value is instead logged as a splat. Behind the scenes, the splat is stored
as a single value rather than a full array. Then, when doing the join at lookup time the splat is repeated across
every instance in the batch.




