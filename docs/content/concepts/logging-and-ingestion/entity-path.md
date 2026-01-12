---
title: The Entity Path Hierarchy
order: 300
---

## Entity paths
As mentioned in the [Entity Component](entity-component.md) overview, all entities within Rerun have a unique _entity path_.

The first argument to the `log()` function is this path. Each time you log to a specific entity path you will update the entity, i.e. log a new instance of it along the timeline.

It is possible to log multiple types of archetypes on the same entity path, but you should generally avoid mixing different kinds of geometric primitive. For example, logging a [`Points3D`](../../reference/types/archetypes/points3d.md) point cloud on an entity path where a [`Mesh3D`](../../reference/types/archetypes/mesh3d.md) was previously logged would overwrite the mesh's [`Position3D`](../../reference/types/components/position3d.md) component with the point cloud's, but would leave the `triangle_indices` component untouched. The Rerun Viewer would likely be unable to display the result. See the [Entity Component](entity-component.md) section for more information.

There _are_ valid reasons to logs different kinds of archetypes to the same entity path, though. For example, it's common to log a [`Transform3D`](../../reference/types/archetypes/transform3d.md) along with some geometry it relates to (see the [Transforms & Coordinate Frames](transforms.md) for more info).

Rerun treats entity paths as being arranged in a hierarchy with the `/` character acting as a separator between path
elements. The conventional path semantics including concepts of *root* and *parent*/*child* generally apply.

When writing paths in logging APIs the leading `/` is usually omitted.

In the file path analogy, each entity is a folder, and a component is a file.
This implies that any entity in a hierarchy can contain components.

For example (this uses the Python SDK but the same applies for all supported languages):

```python
rr.log("image", rr.Image(img))
rr.log("image/points", rr.Points2D(points))
```

It is also acceptable to leave implicitly "empty" entities in your paths as well.
```python
rr.log("camera/image", rr.Image(img))
rr.log("camera/image/detections/points", rr.Points2D(points))
```

Nothing needs to be explicitly logged to `"camera"` or `"camera/image/detection"` to make the above valid.
In other words, the `log` call is akin to creating a folder with `mkdir -p` and then writing files (components) to it.
Existing components of the same name will be overwritten.

### Path parts

Each "part" of a path must be a non-empty string. Any character is allowed, but special characters need to be escaped using `\`.
Characters that need NOT be escaped are letters, numbers, and underscore, dash, and dot (`_`, `-`, `.`).
Any other character should be escaped, including symbols (`\:`, `\$`, …) and whitespace (`\ `, `\n`, `\t`, …).

You can insert an arbitrary unicode code point into an entity path using `\u{262E}`.

So for instance, `world/3D/My\ Image.jpg/detection` is a valid path (note the escaped space!).

⚠️ NOTE: even though entity paths are somewhat analogous to file paths, they are NOT the same. `..` does not mean "parent folder", and you are NOT intended to pass a file path as an entity path (especially not on Windows, which use `\` as a path separator).

### Path hierarchy functions
Path hierarchy plays an important role in a number of different functions within Rerun:

 * With the [Transform System](transforms.md) the `transform` component logged to any entity always describes
the relationship between that entity and its direct parent.
 * When resolving the meaning of [`ClassId`](../../reference/types/components/class_id.md) and [`KeypointId`](../../reference/types/components/keypoint_id.md) components, Rerun uses the [Annotation Context](../visualization/annotation-context.md) from the nearest ancestor in the hierarchy.
 * When adding data to [Blueprints](../../reference/viewer/blueprints.md), it is common to add a path and all of its descendants.
 * When using `rr.log("entity/path", rr.Clear(recursive=True))`, it marks an entity *and all of its descendants* as being cleared.
 * In the future, it will also be possible to use path-hierarchy to set default-values for descendants
   ([#1158](https://github.com/rerun-io/rerun/issues/1158)).

### Reserved paths

The path prefix `__` is considered reserved for use by the Rerun SDK itself and should not be used for logging
user data. This is where Rerun will log additional information such as properties (`__properties`) and warnings
(`__warnings`).
