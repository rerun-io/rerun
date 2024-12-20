---
title: Migrating from 0.8 to 0.9
order: 1000
---

Rerun-0.9 introduces a new set of type-oriented logging APIs built on top of an updated, more concrete,
[data model](../../concepts/entity-component.md).

Rather than using different functions to log different kinds of data, all data logging now goes through a singular `log`
function. The easiest way to use the `log` function is with the Rerun-provided "Archetypes."

Archetypes are a newly introduced concept in the data model to go alongside "Components" and "DataTypes." Archetypes
represent common objects that are natively understood by the viewer, e.g. `Image` or `Points3D`. Every legacy logging
API has been replaced by one (or more) new Archetypes. You can find more information in the [Entity Component](../../concepts/entity-component.md) section, and a list of available archetypes in the
[Archetype Overview](../types/archetypes.md). All Archetypes are part of the top-level `rerun` namespace.

In practice, the changes are mostly demonstrated in the following example:

snippet: migration/log_line

Note that for any Archetype that supports batching the object names are now plural. For example, points are now logged
with the `Points3D` archetype. Even if you are logging a single point, under the hood it is always implemented as a
batch of size 1.

For more information on the relationship between Archetypes, Components, and DataTypes, please see our guide to the [Rerun Data Model](../../concepts/entity-component.md).

# Migrating Python code

All of the previous `log_*` functions have been marked as deprecated and will be removed in `0.10`. We have done our
best to keep these functions working as thin wrappers on top of the new logging APIs, though there may be subtle
behavioral differences.

## The log module has become the log function
This is one area where we were forced to make breaking changes. Rerun previously had an internal `log` module where the
assorted log-functions and helper classes were implemented. In general, these symbols were all re-exported to the
top-level `rerun` namespace. However, in some cases these fully-qualified paths were used for imports. Because
`rerun.log` is now a function rather than a module, any such imports will result in an import error. Look for the
corresponding symbol in the top-level `rerun` namespace instead. For instance: `rr.log.text.LoggingHandler` → `rr.LoggingHandler`

## Updating to the log APIs

In most cases migrating your code to the new APIs should be straightforward. The legacy functions have been marked as
deprecated and the deprecation warning should point you to the correct Archetype to use instead. Additionally, in most
cases, the old parameter names match the parameters taken by the new Archetype constructors, though exceptions are noted below.

### log_annotation_context
Replace with [AnnotationContext](../types/archetypes/annotation_context.md)

Python docs: [AnnotationContext](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.AnnotationContext)

Notes:
 - `class_descriptions` has become `context`
 - `rr.ClassDescription` now requires `info` to be provided rather than defaulting to 0.
 - `rr.AnnotationInfo` now requires `id` to be provided rather than defaulting to 0.

### log_arrow
Replace with [Arrows3D](../types/archetypes/arrows3d.md)

Python docs: [Arrows3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Arrows3D)

Notes:
 - `with_scale` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### log_cleared
Replace with [Clear](../types/archetypes/clear.md)

Python docs: [Clear](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Clear)

### log_depth_image
Replace with [DepthImage](../types/archetypes/depth_image.md)

Python docs: [DepthImage](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Clear)

Notes:
 * `image` has become `data`

### log_disconnected_space
Replace with `DisconnectedSpace`

Python docs: [DisconnectedSpace](https://ref.rerun.io/docs/python/0.21.0/common/archetypes/#rerun.archetypes.DisconnectedSpace)

### log_extension_components
Replace with `AnyValues`

Python docs: [AnyValues](https://ref.rerun.io/docs/python/stable/common/custom_data/#rerun.AnyValues)

Notes:
 - Instead of passing `ext` as a dictionary, `AnyValues` now maps all keyword arguments directly to components.
   - `rr.log_extension_components(…, ext={'mydata': 1})` becomes `rr.log(… rr.AnyValues(mydata=1))`

### log_image
Replace with [Image](../types/archetypes/image.md)

Python docs: [Image](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Image)

Notes:
 * `image` has become `data`
 * `jpeg_quality` is now handled by calling `.compress(jpeg_quality=…)` on the image after constructing it.

### log_image_file
Replace with `ImageEncoded`

Python docs: [ImageEncoded](https://ref.rerun.io/docs/python/stable/common/image_helpers/#rerun.ImageEncoded)

Notes:
 - `img_bytes` and `img_path`


### log_line_strip, log_line_strips_2d, log_line_strips_3d, log_line_segments
Replace with [LineStrips2D](../types/archetypes/line_strips2d.md) or [LineStrips3D](../types/archetypes/line_strips3d.md)

Python docs: [LineStrips2D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.LineStrips2D), [LineStrips3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.LineStrips3D)

Notes:
 - `log_line_segments` used to take an array of shape (2 * num_segments, 2 or 3) (where points were connected in
even-odd pairs). Instead this is now handled by a batch of `LineStrips` all of length 2. Note that `LineStrips` now
takes any sequence of arrays of shape (num_points_per_strip, 2 or 3). You can use convert to the new format using the
snippets:
```
line_strips2d=line_segments.reshape(-1, 2, 2)
line_strips3d=line_segments.reshape(-1, 2, 3)
```
 - `positions` has become `strips`.
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### log_mesh, log_meshes
Replace with [Mesh3D](../types/archetypes/mesh3d.md)

Python docs: [Mesh3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Mesh3D)

Notes:
 - Meshes are no longer batch objects. Instead they are treated as a batch of vertices, as such there is no longer a
   direct equivalent of `log_meshes`.
 - `positions` has become `vertex_positions`.
 - `normals` has become `vertex_normals`.
 - `albedo_factor` has become `mesh_material`, and can be logged using `rr.Material(albedo_factor=…)`.
 - `identifiers` has become `instance_keys`.

### log_mesh_file
Replace with [Asset3D](../types/archetypes/asset3d.md)

Python docs: [Asset3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Asset3D)

Notes:
 - `mesh_bytes` and `mesh_path` are both now jut `data`. Strings and paths will be opened as files, while
   file-descriptors or bytes objects will be read.
 - `mesh_format` is now `media_type`.
 - `transform` can now take anything that is compatible with `rr.Transform3D` instead of an affine 3x4 matrix.
   - To convert an existing affine 3x4 matrix to an `rr.Transform3D`, you can use, `rr.Transform3D(translation=transform[:,3], mat3x3=transform[:,0:3])`

### log_obb, log_obbs
Replace with [Boxes3D](../types/archetypes/boxes3d.md)

Python docs: [Boxes3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Boxes3D)

Notes:
 - `positions` has become `centers`.
 - `rotations_q` has become `rotations` and can now take any `Rotation3DArrayLike` such as `rr.Quaternion` or
   `rr.RotationAxisAngle`.
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### log_pinhole
Replace with [Pinhole](../types/archetypes/pinhole.md)

Python docs: [Pinhole](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Pinhole)

Notes:
 - `child_from_parent` has become `image_from_parent`.
 - `focal_length_px` has become `focal_length`.
 - `principal_point_px` has become `principal_point`.
 - New argument `resolution` to specify width and height using `Vec2D`
 - `camera_xyz` no longer take a string. Now use one of the constants from `rr.ViewCoordinates`

### log_point, log_points
Replace with [Points2D](../types/archetypes/points2d.md) or [Points3D](../types/archetypes/points3d.md).

Python docs: [Points2D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points2D), [Points3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Points3D)

Notes:
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`

### log_rect, log_rects
Replace with [Boxes2D](../types/archetypes/boxes2d.md)

Python docs: [Boxes2D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Boxes2D)

Notes:
 - Can now be constructed with 2 arrays: `centers`, and either `half_sizes` o `sizes`.
    - The legacy behavior of a single array can be matched by using the params `array` and `array_format`.
   `array_format` takes an `rr.Box2DFormat`.
 - `identifiers` has become `instance_keys`.

### log_scalar
Replace with `TimeSeriesScalar`

### log_segmentation_image
Replace with [SegmentationImage](../types/archetypes/segmentation_image.md)

Python docs: [SegmentationImage](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.SegmentationImage)

Notes:
 * `image` has become `data`

### log_tensor
Replace with [Tensor](../types/archetypes/tensor.md)

Python docs: [Tensor](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Tensor)

Notes:
 - `tensor` has become `data`.
 - `names` has become `dim_names`.
 - `meter` is no longer supported -- use `rr.DepthImage` instead.
 - 1D Tensors can now be logged with [BarChart](../types/archetypes/bar_chart.md)


### log_text_entry
Replace with [TextLog](../types/archetypes/text_log.md)

Python docs: [TextLog](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.TextLog)

### log_transform3d
Replace with [Transform3D](../types/archetypes/transform3d.md)

Python docs: [Transform3D](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.Transform3D)

Notes:
 - Now takes optional parameters for `translation`, `rotation`, `scale`, or `mat3x3` to simplify construction.

### log_view_coordinates
Replace with [ViewCoordinates](../types/archetypes/view_coordinates.md)

Python docs: [ViewCoordinates](https://ref.rerun.io/docs/python/stable/common/archetypes/#rerun.archetypes.ViewCoordinates)

Notes:
- Rather than providing `xyz` or `up` as strings, `rr.ViewCoordinates` exposes a large number of constants that can be logged directly. For example: `rr.ViewCoordinates.RDF` or `rr.ViewCoordinates.RIGHT_HAND_Z_DOWN)`


# Migrating Rust code

Rust already used a more type oriented interface, so the changes are not as drastic as to the Python API.

## Removal of MsgSender

The biggest change that `MsgSender` is gone and all logging happens instead directly on the [`RecordingStream::RecordingStream`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html)
using its [`log`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log) and [`RecordingStream::log_timeless`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_timeless) functions.

## Logging time

The new `log` function logs time implicitly. `log_time` and `log_tick` are always included, as well as any custom timeline set using [`RecordingStream::set_timepoint`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_timepoint), or one of the shorthands [`RecordingStream::set_time_sequence`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_time_sequence)/[`RecordingStream::set_time_seconds`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_time_seconds)/[`RecordingStream::set_time_nanos`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.set_time_nanos)

## Components -> archetypes

The new log messages consume any type that implements the [`AsComponents`](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html) trait
which is [implemented by](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html#implementors) all archetypes.
All previously separately logged components have corresponding types and are used in one or more archetypes.
See the respective API docs as well as the [Archetype Overview](../types/archetypes.md) to learn more and find self-contained code examples.

For continuing to log collections of components without implementing the [`AsComponents`](https://docs.rs/rerun/latest/rerun/trait.AsComponents.html) trait, use [`RecordingStream::log_component_batches`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_component_batches)



## Splatting

Splatting is no longer done explicitly (before `MsgSender::splat`), but automatically inferred whenever
there is a single component together with larger component batches on the same entity path.
See also [`RecordingStream::log_component_batches`](https://docs.rs/rerun/latest/rerun/struct.RecordingStream.html#method.log_component_batches) for more information.
