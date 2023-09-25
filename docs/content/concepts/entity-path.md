---
title: The Entity Path Hierarchy
order: 1
---

## Entity Paths
As mentioned in the [Entity Component](entity-component.md) overview, all Entities within Rerun have a unique _Entity Path_.

The first argument to each log function is this path. Each time you log to a specific entity path you will update the entity, i.e. log a new instance of it along the timeline. Each logging to a path must be of the same type (you cannot log an image to the same path as a point cloud).

Rerun treats these paths as being arranged in a hierarchy with the "/" character acting as a separator between path
elements. The conventional path semantics including concepts of "root" and "parent" / "child" generally apply.

When writing paths in logging APIs the leading "/" is omitted.

Note that there is no path-level distinction between "file-like" and "directory-like" concepts. Any path may be an
entity, and entities may be direct children of other entities. For example:
```
rr.log_image("image", img)
rr.log_points("image/points", points)
```

However, it is also acceptable to leave implicitly "empty" elements in your paths as well.
```
rr.log_image("camera/image", img)
rr.log_points("camera/image/detections/points", points)
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

 * With the [Transform System](spaces-and-transforms.md) the `transform` component logged to any Entity always describes
the relationship between that Entity and its direct parent.
 * When resolving the meaning of Class ID and Keypoint ID components, Rerun uses the [Annotation Context](annotation-context.md) from the nearest ancestor in the hierarchy.
 * When adding data to [Blueprints](../reference/viewer/blueprint.md), it is common to add a path and all of its descendants.
 * When using the `log_cleared` API, it is possible to mark an entity and all of its descendants as being cleared.
 * In the future, it will also be possible to use path-hierarchy to set default-values for descendants.
   [#1158](https://github.com/rerun-io/rerun/issues/1158)

