---
title: The Entity Path Hierarchy
order: 300
description: How entity paths organize logged data into a hierarchy you can query
---

## Entity paths
As mentioned in the [Entity Component](entity-component.md) overview, all entities within Rerun have a unique _entity path_.

The first argument to the `log()` function is this path. Each time you log to a specific entity path you will update the entity, i.e. log a new instance of it along the timeline.

It is possible to log multiple archetypes on the same entity path.
Each component is tagged with its archetype (e.g. [`Points3D`](../../reference/types/archetypes/points3d.md)`:positions` vs. [`Mesh3D`](../../reference/types/archetypes/mesh3d.md)`:vertex_positions`), so they don't overwrite each other.
See the [Entity Component](entity-component.md) section for more information.

For example, it's common to log a [`Transform3D`](../../reference/types/archetypes/transform3d.md) along with some geometry it relates to (see the [Transforms & Coordinate Frames](transforms.md) for more info).

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

Nothing needs to be explicitly logged to `"camera"` or `"camera/image/detections"` to make the above valid.
In other words, the `log` call is akin to creating a folder with `mkdir -p` and then writing files (components) to it.
Existing components of the same name will be overwritten.

### Path parts

Each "part" of a path must be a non-empty string. Any character is allowed, but special characters need to be escaped using `\`.
Characters that need NOT be escaped are letters, numbers, and underscore, dash, and dot (`_`, `-`, `.`).
Any other character should be escaped, including symbols (`\:`, `\$`, …) and whitespace (`\ `, `\n`, `\t`, …).

You can insert an arbitrary unicode code point into an entity path using `\u{262E}`.

So for instance, `world/3D/My\ Image.jpg/detection` is a valid path (note the escaped space!).

> [!WARNING]
> Even though entity paths are somewhat analogous to file paths, they are NOT the same. `..` does not mean "parent folder", and you are NOT intended to pass a file path as an entity path (especially not on Windows, which uses `\` as a path separator).

### Path hierarchy functions
Path hierarchy plays an important role in a number of different functions within Rerun:

 * With the [Transform System](transforms.md), a `Transform3D` logged to an entity without explicit `parent_frame`/`child_frame` describes
the relationship between that entity and its direct parent.
 * When resolving the meaning of [`ClassId`](../../reference/types/components/class_id.md) and [`KeypointId`](../../reference/types/components/keypoint_id.md) components, Rerun uses the [Annotation Context](../../viewer/blueprints-and-views/annotation-context.md) from the nearest ancestor in the hierarchy.
 * When adding data to [Blueprints](../../viewer/ui/blueprint-panel.md), it is common to add a path and all of its descendants.
 * When using `rr.log("entity/path", rr.Clear(recursive=True))`, it marks an entity *and all of its descendants* as being cleared.
 * In the future, it will also be possible to use path-hierarchy to set default-values for descendants
   ([#1158](https://github.com/rerun-io/rerun/issues/1158)).

### Entity path filters

An entity path filter selects entities with a list of expressions.
Rerun uses them for [view contents](../../viewer/blueprints-and-views/entity-queries.md), `filter_contents()` on [datasets](../../query-and-transform/dataframes/dataframe-queries.md), and the `content` argument of the [chunk processing API](../import/chunk-processing-api.md).

Each expression is either an inclusion, which starts with an optional `+`, or an exclusion, which always starts with a `-`.

Expressions are also allowed to end with an optional `/**`.
The `/**` suffix matches the whole subtree, i.e. self and any child, recursively.
For example, `/world/**` matches both `/world` and `/world/car/driver`.
Other uses of `*` are not yet supported.

When combining multiple expressions, the rules are sorted by entity path, from least to most specific:

- If there are multiple matching rules, the most specific rule wins.
- If there are multiple rules of the same specificity, the last one wins.
- If no rules match, the path is excluded.

Consider the following example:

```diff
+ /world/**
- /world
- /world/car/**
+ /world/car/driver
```

- The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
- The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
- The last rule matching `/world` is `- /world`, so it is excluded.
- The last rule matching `/world/house` is `+ /world/**`, so it is included.

### Reserved paths

The path prefix `__` is considered reserved for use by the Rerun SDK itself and should not be used for logging
user data. This is where Rerun will log additional information such as properties (`__properties`) and warnings
(`__warnings`).

### Performance considerations

There is no hard limit on the number of entities, but each entity has a cost in the Viewer: it is queried, stored in its own chunks, and listed in the UI.
Many thousands of entities make the Viewer slow, even if each one holds little data.

- **Batch instances into one entity.**
  Log a point cloud as one `Points3D` entity with many points, not one entity per point.
  The same goes for boxes, arrows, and other batched archetypes.
- **Use entities for things you want to see, hide, or style separately.**
  For example, one entity per tracked object or per sensor.
- **Avoid unbounded entity names**, such as an entity per timestamp or per message ID.
  Use a [timeline](timelines.md) for the time, and keep the entity path stable.

See also [Optimize chunk count](../chunks-and-rrd/optimize-chunks.md).
