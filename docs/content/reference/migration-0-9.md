---
title: Migration to 0.9
order: 10
---

Rerun-0.9 introduces a new set of object-oriented logging APIs built on top of an updated, more concrete,
[data model](../concepts/entity-component.md).

Rather than using different functions to log different kinds of data, all data logging now goes through a singular `log`
function. The easiest way to use the `log` function is with the Rerun-provided "Archetypes."

Archetypes are a newly introduced concept in the data model to go alongside "Components" and "DataTypes." Archetypes
represent common objects that are natively understood by the viewer, e.g. `Image` or `Points3D`. Every legacy logging
API has been replaced by one (or more) new Archetypes. You can find all of the available archetypes in the
[Archetype Overview](data_types/archetypes.md). All Archetypes are part of the top-level `rerun` namespace.

In practice, the changes are mostly demonstrated in the following example:

code-example: log_line

Note that for any Archetype that supports batching the object names are now plural. For example, points are now logged
with the `Points3D` archetype. Even if you are logging a single point, under the hood it is always implemented as a
batch of size 1.

For more information on the relationship between Archetypes, Components, and DataTypes, please see our guide to the [Rerun Data Model](../concepts/entity-component.md).

# Migrating Python Code

All of the previous `log_*` functions have been marked as deprecated and will be removed in `0.10`. We have done our
best to keep these functions working as thin wrappers on top of the new logging APIs, though there may be subtle
behavioral differences.

## The log module has become the log function
This is one area where we were forced to make breaking changes.  Rerun previously had an internal `log` module where the
assorted log-functions and helper classes were implemented. In general, these symbols were all re-exported to the
top-level `rerun` namespace.  However, in some cases these fully-qualified paths were used for imports. Because
`rerun.log` is now a function rather than a module, any such imports will result in an import error. Look for the
corresponding symbol in the top-level `rerun` namespace instead.

## Updating to the log APIs

In most cases migrating your code to the new APIs should be straightforward. The legacy functions have been marked as
deprecated and the deprecation warning should point you to the correct Archetype to use instead.  Additionally, in most
cases, the old parameter names match the parameters taken by the new Archetype constructors, though exceptions are noted below.

### `log_annotation_context`
Replace with [AnnotationContext](data_types/archetypes/annotation_context.md)

Python docs: [AnnotationContext.__init__](https://ref.rerun.io/docs/python/HEAD/common/annotations/#rerun.AnnotationContext.__init__)

Notes:
 - `class_descriptions` has become `context`
 - `rr.ClassDescription` now requires `info` to be provided rather than defaulting to 0.
 - `rr.AnnotationInfo` now requires `id` to be provided rather than defaulting to 0.

### `log_arrow`
Replace with [Arrows3D](data_types/archetypes/arrows3d.md)

Python docs: [Arrows3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Arrows3D.__init__)

Notes:
 - `with_scale` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### `log_cleared`
Replace with [Clear](data_types/archetypes/clear.md)

Python docs: [Clear.__init__](https://ref.rerun.io/docs/python/HEAD/common/clearing_entities/#rerun.Clear.__init__)

### `log_depth_image`
Replace with [DepthImage](data_types/archetypes/depth_image.md)

Python docs: [DepthImage.__init__](https://ref.rerun.io/docs/python/HEAD/common/images/#rerun.DepthImage.__init__)

Notes:
 * `image` has become `data`

### `log_disconnected_space`
Replace with [DisconnectedSpace](data_types/archetypes/disconnected_space.md)

Python docs: [DisconnectedSpace.__init__](https://ref.rerun.io/docs/python/HEAD/common/transforms_and_coordinate_systems/#rerun.DisconnectedSpace.__init__)

### `log_extension_components`
Replace with `AnyValues`

Python docs: [AnyValues.__init__](https://ref.rerun.io/docs/python/HEAD/common/adhoc_data/#rerun.AnyValues.__init__)

Notes:
 - Instead of passing `ext` as a dictionary, `AnyValues` now maps all keyword arguments directly to components.
   - `rr.log_extension_components(..., ext={'mydata': 1})` becomes `rr.log(... rr.AnyValues(mydata=1))`

### `log_image`
Replace with [Image](data_types/archetypes/image.md)

Python docs: [Image.__init__](https://ref.rerun.io/docs/python/HEAD/common/images/#rerun.Image.__init__)

Notes:
 * `image` has become `data`
 * `jpeg_quality` is now handled by calling `.compress(jpeg_quality=...)` on the image after constructing it.

### `log_image_file`
Replace with `ImageEncoded`

Python docs: [ImageEncoded.__init__](https://ref.rerun.io/docs/python/HEAD/common/images/#rerun.ImageEncoded.__init__)

Notes:
 - `img_bytes` and `img_path`


### `log_line_strip`, `log_line_strips_2d`, `log_line_strips_3d`, `log_line_segments`
Replace with [Lines2D](data_types/archetypes/lines2d.md) or [Lines3D](data_types/archetypes/lines3d.md)

Python docs: [Lines2D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Lines2D.__init__), [Lines3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Lines3D.__init__)

Notes:
 - `log_line_segments` used to take an array of shape (2 * num_segments, 2 or 3) (where points were connected in
even-odd pairs). Instead this is now handled by a batch of `LineStrips` all of length 2. Note that `LineStrips` now
takes an array of shape (num_strips, num_points_per_strip, 2 or 3). You can use convert to the new format using the snippets:
```
line_strips2d=line_segments.reshape(-1, 2, 2)
line_strips3d=line_segments.reshape(-1, 2, 3)
```
 - `positions` has become `strips`.
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### `log_mesh`, `log_meshes`
Replace with [Mesh3D](data_types/archetypes/meshes3d.md)

Python docs: [Mesh3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Meshes3D.__init__)

Notes:
 - Meshes are no longer batch objects. Instead they are treated as a batch of vertices, as such there is no longer a
   direct equivalent of `log_meshes`.
 - `positions` has become `vertex_positions`.
 - `normals` has become `vertex_normals`.
 - `albedo_factor` has become `mesh_material`, and can be logged using `rr.Material(albedo_factor=...)`.
 - `identifiers` has become `instance_keys`.

### `log_mesh_file`
Replace with [Asset3D](data_types/archetypes/assets3d.md)

Python docs: [Asset3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Asset3D.__init__)

Notes:
 - `mesh_bytes` and `mesh_path` are both now jut `data`. Strings and paths will be opened as files, while
   file-descriptors or bytes objects will be read.
 - `mesh_format` is now `media_type`.
 - `transform` can now take anything that is compatible with `rr.Transform3D` instead of a 3x4 matrix.

### `log_obb`, `log_obbs`
Replace with [Boxes3D](data_types/archetypes/boxes3d.md)

Python docs: [Boxes3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Boxes3D.__init__)

Notes:
 - `positions` has become `centers`.
 - `rotations_q` has become `rotations` and can now take any `Rotation3DArrayLike` such as `rr.Quaternion` or
   `rr.RotationAxisAngle`.
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`.

### `log_pinhole`
Replace with [Pinhole](data_types/archetypes/pinhole.md)

Python docs: [Pinhole.__init__](https://ref.rerun.io/docs/python/HEAD/common/transforms_and_coordinate_systems/#rerun.Pinhole.__init__)

Notes:
 - `child_from_parent` has become `image_from_parent`.
 - `focal_length_px` has become `focal_length`.
 - `principal_point_px` has become `principal_point`.
 - New argument `resolution` to specify width and height using `Vec2D`
 - `camera_xyz` no longer take a string. Now use one of the constants from `rr.ViewCoordinates`

### `log_point`, `log_points`
Replace with [Points2D](data_types/archetypes/points2d.md) or [Points3D](data_types/archetypes/points3d.md).

Python docs: [Points2D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Points2D.__init__), [Points3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Points3D.__init__)

Notes:
 - `stroke_width` has become `radii`, which entails dividing by 2 as necessary.
 - `identifiers` has become `instance_keys`

### `log_rect`, `log_rects`
Replace with [Boxes2D](data_types/archetypes/boxes2d.md)

Python docs: [Boxes2D.__init__](https://ref.rerun.io/docs/python/HEAD/common/spatial_archetypes/#rerun.Boxes2D.__init__)

Notes:
 - Can now be constructed with 2 arrays: `centers`, and either `half_sizes` o `sizes`.
    - The legacy behavior of a single array can be matched by using the params `array` and `array_format`.
   `array_format` takes an `rr.Box2DFormat`.
 - `identifiers` has become `instance_keys`.

### `log_scalar`
Replace with [TimeSeriesScalar](data_types/archetypes/time_series_scalar.md)

Python docs: [TimeSeriesScalar.__init__](https://ref.rerun.io/docs/python/HEAD/common/plotting/#rerun.TimeSeriesScalar.__init__)

### `log_segmentation_image`
Replace with [SegmentationImage](data_types/archetypes/segmentation_image.md)

Python docs: [SegmentationImage.__init__](https://ref.rerun.io/docs/python/HEAD/common/images/#rerun.SegmentationImage.__init__)

Notes:
 * `image` has become `data`

### `log_tensor`
Replace with [Tensor](data_types/archetypes/tensor.md)

Python docs: [Tensor.__init__](https://ref.rerun.io/docs/python/HEAD/common/tensors/#rerun.Tensor.__init__)

Notes:
 - `tensor` has become `data`.
 - `names` has become `dim_names`.
 - `meter` is no longer supported -- use `rr.DepthImage` instead.
 - 1D Tensors can now be logged with [BarChart](data_types/archetypes/bar_chart.md)


### `log_text_entry`
Replace with [TextLog](data_types/archetypes/text_log.md)

Python docs: [TextLog.__init__](https://ref.rerun.io/docs/python/HEAD/common/text/#rerun.TextLog.__init__)

### `log_transform3d`
Replace with [Transform3D](data_types/archetypes/transform3d.md)

Python docs: [Transform3D.__init__](https://ref.rerun.io/docs/python/HEAD/common/transforms_and_coordinate_systems/#rerun.Transform3D.__init__)

Notes:
 - Now takes an optional parameters for `translation`, `rotation`, `scale`, or `mat3x3` to simplify construction.

### `log_view_coordinates`
Replace with [ViewCoordinates](data_types/archetypes/view_coordinates.md)

Python docs: [ViewCoordinates.__init__](https://ref.rerun.io/docs/python/HEAD/common/transforms_and_coordinate_systems/#rerun.ViewCoordinates.__init__)

Notes:
- Rather than providing `xyz` or `up` as strings, `rr.ViewCoordinates` exposes a large number of constants that can be logged directly. For example: `rr.ViewCoordinates.RDF` or `rr.ViewCoordinates.RIGHT_HAND_Z_DOWN)`

