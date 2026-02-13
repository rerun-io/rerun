---
title: Component Batches
order: 900
---

In the Rerun data model, the value of a given component at a given point in time is always itself a list—or a _batch_—of values.

Consider this example:

```python
rr.log("/data", rr.Points3D(positions=[0.0, 0.0, 0.0]))
```

For convenience, the [`rr.Points3D`](../../reference/types/archetypes/points3d.md) archetype accepts a single position, but what actually happens is that the corresponding [`Position3D`](../../reference/types/components/position3d.md) component is logged as a batch of length 1.
So the following log calls are equivalent:
```python
single_point = [0.0, 0.0, 0.0]
rr.log("/data", rr.Points3D(positions=single_point)
rr.log("/data", rr.Points3D(positions=[single_point])
```

Logging larger batches is obviously possible:

```python
rr.log("/data", rr.Points3D(positions=[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]))
```

The ability to log data as batches is useful in many cases, such as point clouds (as in the above example), bounding boxes for detected objects, tracked keypoints in a skeleton, or individual joint values for a robot arm.

This is also why, in the logging APIs, the majority of archetypes are named with the plural form, like `rr.Points3D` above.

An individual value within a batch is called an _instance_.


## Component batches are immutable

When data is logged to a component for a given time point, the corresponding batch is immutable.
This means that additional instances cannot be appended to it, and existing instances cannot be modified.
The entire batch must be logged again, and this will replace the previous one.

Note that when data is logged multiple times for the same component and at the same time point, the last logged batch will be used, but the previously logged batches will remain in storage.

## Instance joining semantics

Components are typically logged as part of archetypes, which are semantic groupings of related components (see [Entities and Components](entity-component.md)).
Often, archetypes have instance joining semantics.
This means that the nth instance of one of the components relates to the nth instance of other components.
For example, this is the case of [`rr.Points3D`](../../reference/types/archetypes/points3d.md): the nth value of its `colors` field applies to the nth value of its `positions` field.

### Instance clamping

Such archetypes typically have a required component that acts as the _primary component_.
That's the component which defines how many logical instances the logged archetype represents.
For `rr.Points3D`, the primary component is [`Position3D`](../../reference/types/components/position3d.md).
Its batch size determines how many points will be visible in the viewer.

For components other than the primary component:
- if they have more instances, the additional instances are ignored by the viewer;
- if they have fewer instances, the last instance is repeated as required.

We refer to the latter case as _clamping semantics_, which can also be seen as a left-join using the primary component.

This enables natural logging calls such as the following:

```python
rr.log("/data", rr.Points3D(positions=[[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]], radii=0.5))
```

Here, an N=3 batch of positions is logged, along with a batch of N=1 radii.
That unique radius value is clamped to the three positions and thus applies to all three points when displayed in the viewer.

### Instance joining and latest-at semantics

Instance joining applies to the _current_ value of the components being displayed in the viewer.
It is worth remembering that the [latest-at semantics](latest-at.md) still apply, which means that joined components do not need to be logged at the same time.

For example, one might log a point cloud with positions and colors at the beginning of a recording, and later only log updated positions.
The viewer will always look up for the "last" colors that were logged ("latest at" semantics) and use them for display.

### Instance joining is not universal

Note that instance joining semantics are not universal.
Some archetypes don't use it, or use it partially.

For example, the [`rr.Mesh3D`](../../reference/types/archetypes/mesh3d.md) archetype has a `vertex_positions` required component, which defines the number of vertices in the mesh.
Some components have instance joining semantics with `vertex_positions`, including `vertex_colors` and `vertex_texcoords`.
However, some other components do not, including `triangle_indices` which contains triplets of indices into the `vertex_positions` batch and defines the triangles to be displayed.


## Storage

Internally, component data is stored as [Arrow List arrays](https://arrow.apache.org/docs/format/Columnar.html#variable-size-list-layout) within [chunks](chunks.md).
Each row of the list array corresponds to a single time point, and the values in each row correspond to the component batch.
The Rerun data model exploits the fact that list arrays can have different lengths in each row to allow component batches to have different lengths at each time point.

This design choice is most visible when [querying Rerun data](../query-and-transform/dataframe-queries.md).
The returned dataframes will always have the `ListArray` datatype for component columns, even if the underlying columns contain a single value per row, or all rows (or batches) have the same length.


## See also

[`send_columns`](../../howto/logging-and-ingestion/send-columns.md) lets you efficiently send many batches of data in one log call.

