---
title: Batch Data
order: 4
---

Rerun has built-in support for batch data. Whenever you have a collection of things that all have the same type, rather
than logging each element individually, you can log the entire collection together as a single "Batch". This provides
significant benefits in terms of storage and compute.

Some examples of batched data include points in a pointcloud, bounding boxes for detected objects, or tracked keypoints
in a skeleton.

In the Python APIs most of the log functions have both singular and plural signatures. The plural APIs, such as
[log_points](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points) generally take
arrays as arguments, corresponding to the batches.

## Terminology
- When an entity is batched, the individual elements are called **"Instances"**.
- Each Instance is identified within the Entity by its **"Instance Key"**.
- When every Instance within an Entity has the same value for a component, that component is called a **"Splat"**. This
  is a common enough case that there is special handling for it.
  For instance, you can set all the colors of a point cloud to the same color using a Splat.
- During queries, a batch always has a **"Primary"** component. The primary component is what determines
  how many instances exist in the batch.

## Restrictions

When using batched entities there are a few restrictions:
 - Because there is a one-to-one mapping between Batches and Entities:
    - If data needs to span multiple Entity Paths, it needs to be split up into separate batches.
    - If data needs to be split into multiple batches, each must be logged to a different path.
 - Whenever you log a batched Entity, for any component that is updated, you must provide values for
   every instance.
    - It is not possible to only update a subset of the instances in the batch.

## Batch join rules

Rerun lets you choose which components in an entity you want to log at any point in time. If you don't log to a
component, then in general it is not updated. For example, if you log a point cloud with positions and colors and then
later log just new positions, when the viewer displays that point cloud it will still look up the *last* colors that
were logged.

This can be quite convenient since updating different components at different times puts far fewer restrictions on the
organization of your code. It even means if a component on an entity is static, you only need to log it once.

However, if a batch of colors, and a batch of positions have been logged at two different points in time, we need a way
to know which point receives which color. This is what Rerun uses the "Instance Keys" for. When a component batch is
logged it is always assigned a set of instance keys. By default, this key is based on the sequential index within the
logged array.  When querying a batched component, the component-values are joined together based on these keys.
Logically, this happens as a **left-join** using the "primary" component for the entity. For example, if you log 3
points and then later log 5 colors, you will still only see 3 points in the viewer.

In the future you will be able to specify the instance keys manually while logging ([#1309](https://github.com/rerun-io/rerun/issues/1309)).

## Splats

As mentioned, Rerun has special handling for "splats" within batches.  This is what happens when you mix arrays and
single values in an API call. The non-array value is instead logged as a splat. Behind the scenes, the splat is stored
as a single value rather than a full array. Then, when doing the join at lookup time the splat is repeated across
every instance in the batch.




