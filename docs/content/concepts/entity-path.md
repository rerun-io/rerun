---
title: The Entity Path Hierarchy
order: 1
---

## Entity Paths
As mentioned in the [Entity Component](entity-component.md) overview, all entities within Rerun have a unique _entity path_.

The first argument to the [`rr.log()`](https://ref.rerun.io/docs/python/HEAD/common/logging/#rerun.log) function is this path. Each time you log to a specific entity path you will update the entity, i.e. log a new instance of it along the timeline.

Note that although it is technically possible to log multiple types of archetypes on the same entity path, it's almost certainly not what you want. For example, logging a [`Points3D`](../reference/types/archetypes/points3d.md) point cloud on an entity path where a [`Mesh3D`](../reference/types/archetypes/mesh3d.md) was previously logged would overwrite the mesh's [`Position3D`](../reference/types/components/position3d.md) component with the point cloud's, but would leave the [`MeshProperties`](../reference/types/components/mesh_properties.md) component untouched. The Rerun viewer would likely be unable to display the result. See the [Entity Component](entity-component.md) section for more information.

Rerun treats entity paths as being arranged in a hierarchy with the `/` character acting as a separator between path
elements. The conventional path semantics including concepts of *root* and *parent*/*child* generally apply.

When writing paths in logging APIs the leading `/` is omitted.

Note that there is no path-level distinction between "file-like" and "directory-like" concepts. Any path may be an
entity, and entities may be direct children of other entities. For example:
```python
rr.log("image", rr.Image(img))
rr.log("image/points", rr.Points2D(points))
```

However, it is also acceptable to leave implicitly "empty" elements in your paths as well.
```python
rr.log("camera/image", rr.Image(img))
rr.log("camera/image/detections/points", rr.Points2D(points))
```
Nothing needs to be explicitly logged to `"camera"` or `"camera/image/detection"` to make the above valid.

#### Path parts

A path can look like this: `camera/"Left"/detection/#42/bbox`. Each part (between the slashes) can either be:

* An identifier (e.g. `camera`), intended for hard-coded names. Only ASCII characters, numbers, underscore, and dash are allowed in identifiers (`[a-zA-Z0-9_-]+`).
* A `"quoted string"`, intended for arbitrary strings, like file names and serials numbers.
* An integer, intended for hashes or similar.
* A number sequence, prefixed by `#`, intended for indices.
* A UUID.

So for instance, `foo/bar/#42/5678/"CA426571"/a6a5e96c-fd52-4d21-a394-ffbb6e5def1d` is a valid path.


### Path Hierarchy Functions
Path hierarchy plays an important role in a number of different functions within Rerun:

 * With the [Transform System](spaces-and-transforms.md) the `transform` component logged to any entity always describes
the relationship between that entity and its direct parent.
 * When resolving the meaning of [`ClassId`](../reference/types/components/class_id.md) and [`KeypointId`](../reference/types/components/keypoint_id.md) components, Rerun uses the [Annotation Context](annotation-context.md) from the nearest ancestor in the hierarchy.
 * When adding data to [Blueprints](../reference/viewer/blueprint.md), it is common to add a path and all of its descendants.
 * When using `rr.log("entity/path", rr.Clear(recursive=True))`, it marks an entity *and all of its descendants* as being cleared.
 * In the future, it will also be possible to use path-hierarchy to set default-values for descendants
   ([#1158](https://github.com/rerun-io/rerun/issues/1158)).

