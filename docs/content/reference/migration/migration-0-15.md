---
title: Migrating from 0.14 to 0.15
order: 995
---

## `InstanceKey` removed from our logging APIs
In PR [#5395](https://github.com/rerun-io/rerun/pull/5395) we removed the `InstanceKey` component from all our archetypes.

What were instance keys?

In Rerun, each entity can be a batch of _instances_.
For instance, a point cloud is usually logged as one entity where each point is an instance of that entity.
An entity is made up of several components (e.g. position, color, …), and you may log these different components separately.
For example, this lets you update only the colors of a point cloud, keeping the same positions.

Instance keys were a way to assign identities to these instances so that you could update the components of a specific set of instances.
This was a complicated and little used feature, that caused far more complexity in our code than it was worth.

For now the `InstanceKey` component as such still remains, but is always assigned the integer index at log time (i.e. `0, 1, 2, 3, …`).
This means that if you first log the positions `A, B, C` and then later log the colors `red, green, blue` to the same entity, they will always be matched as `(A, red), (B, green), (C, blue)`.
We still support _splatting_, where you log one single color for the whole point cloud.

If you were relying on `InstanceKey` solely to identify your instances when inspecting them in the viewer, then you can replace it with a custom value using [custom data](../../howto/logging-and-ingestion/custom-data.md):

```python
rr.log(
    "my/points",
    rr.AnyValues(point_id=[17, 42, 103]),
)
```

In the future we plan on introducing a better way to identify and track instances over time.
