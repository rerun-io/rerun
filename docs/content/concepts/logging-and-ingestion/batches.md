---
title: Component Batches
order: 900
---

Rerun has built-in support for batch data. Whenever you have a collection of things that all have the same type, rather
than logging each element individually, you can log the entire collection together as a single "Batch". This provides
significant benefits in terms of storage and compute.

Some examples of batched data include points in a point cloud, bounding boxes for detected objects, tracked keypoints
in a skeleton, or a collection of line strips.

In the logging APIs, the majority of archetypes are named with the plural form, for example [`rr.Points3D`](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points3D). They accept both single elements (internally treated as an N=1 batch) or arrays corresponding to the batches.

## Terminology

- An *entity* is a collection of *components* (see [Entities and Components](entity-component.md)).
- When an entity is batched, its components' individual elements are called *instances*.
- When every instance within an entity shares the same value for a component, we say that this component is clamped. This
  is a common pattern and has dedicated support for it (see the [Component Clamping](#component-clamping) section below).
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
later log just new positions, when the Viewer displays that point cloud it will still look up the *last* colors that
were logged (we refer to this as the *latest-at* semantics).

This can be quite convenient since updating different components at different times puts far fewer restrictions on the
organization of your code. It even means if a component on an entity is static, you only need to log it once.

However, if both a batch of colors and a batch of positions have been logged at two different points in time, we need a way
to know which point receives which color.
For that, Rerun uses the index of the instance.
When querying a batched component, the component-values are joined together based on this index.
Logically, this happens as a *left-join* using the primary component for the entity. For example, if you log 3
points and then later log 5 colors, you will still only see 3 points in the viewer.

What should happen if you have 5 points and 3 colors then? This is where clamping semantics come into play.

## Component clamping

As mentioned, Rerun has special semantics when joining batches of different sizes, for example this is what happens when you mix arrays and single values in an API call.

If the component on the left-side of the join (the so-called primary component) has more instances than the other, then these tail values will simply be ignored.
On the other hand, if the component on the left-side of the join (the so-called primary component) has less instances than the other, then the last instance will be repeated across every instance left in the batch. We call this clamping, in reference to texture sampling (think `CLAMP_TO_EDGE`!).

## See also
[`send_columns`](../../howto/logging-and-ingestion/send-columns.md) lets you efficiently send many batches of data in one log call.
