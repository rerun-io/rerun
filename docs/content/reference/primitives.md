---
title: Primitives
order: 3
---

To learn what Primitives are, check the concept page on [Entities and Components](../concepts/entity-component.md).

In [Python](https://rerun-io.github.io/rerun) every Primitive is typically backed by one or more function calls.
In contrast, the [Rust API](https://docs.rs/rerun/) works by adding components explicitly.
This is more flexible & extendable but also requires a rough understanding of
which components the Viewer can interpret together as documented below.

## Spatial **Primitives**

### Arrow 3D
* Python: [log_arrow](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_arrow)
* Rust (primary): [Arrow3D](https://docs.rs/rerun/latest/rerun/components/struct.Arrow3D.html)
* Primary component: `arrow3d`
* Secondary components: `colorrgba`, `radius`, `label`

### Rectangle 2D
* Python: [log_rect](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_rect),
[log_rects](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_rects)
* Rust (primary): [Rect2D](https://docs.rs/rerun/latest/rerun/components/enum.Rect2D.html)
* Primary component: `rect2d`, 
* Secondary components: `colorrgba`, `radius`, `label`, `classid`, `draw_order`

### Box 3D
* Python: [log_obb](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_obb)
* Rust (primary): [Box3D](https://docs.rs/rerun/latest/rerun/components/struct.Box3D.html)
* Primary component: `box3d`, 
* Secondary components: `vec3d`, `quaternion`, `colorrgba`, `radius`, `label`, `classid`

### Line 2D
* Python: [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip)
, [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments)
* Rust (primary): [LineStrip2D](https://docs.rs/rerun/latest/rerun/components/struct.LineStrip2D.html)
* Primary component: `linestrip2d`
* Secondary components: `colorrgba`, `radius`, `draw_order`

### Line 3D
* Python: [log_line_strip](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_strip), [log_line_segments](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_line_segments)
* Rust (primary): [LineStrip3D](https://docs.rs/rerun/latest/rerun/components/struct.LineStrip3D.html)
* Primary component: `linestrip3d`
* Secondary components: `colorrgba`, `radius`

### Mesh
* Python: [log_mesh](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_mesh),
[log_meshes](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_meshes),
[log_mesh_file](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_mesh_file)
* Rust (primary): [Mesh3D](https://docs.rs/rerun/latest/rerun/components/enum.Mesh3D.html)
* Primary component: `mesh3d`
* Secondary components: `colorrgba`

### Point 2D
* Python: [log_point](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_point),
[log_points](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points)
* Rust (primary): [Point2D](https://docs.rs/rerun/latest/rerun/components/struct.Point2D.html)
* Primary component: `point2d`
* Secondary components: `colorrgba`, `radius`, `label`, `classid`, `keypointid`, `draw_order`

### Point 3D
* Python: [log_point](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_point),
[log_points](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_points)
* Rust (primary): [Point3D](https://docs.rs/rerun/latest/rerun/components/struct.Point3D.html)
* Primary component: `point3d`
* Secondary components: `colorrgba`, `radius`, `label`, `classid`, `keypointid`

### Transform
* Python: [log_rigid3](https://ref.rerun.io/docs/python/latest/common/transforms/#rerun.log_rigid3),
[log_pinhole](https://ref.rerun.io/docs/python/latest/common/transforms/#rerun.log_pinhole)
* Rust (primary): [Transform](https://docs.rs/rerun/latest/rerun/components/enum.Transform.html)
* Primary component: `transform`

## Tensors & Images

* Python:
[log_tensor](https://ref.rerun.io/docs/python/latest/common/tensors/#rerun.log_tensor),
[log_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image**),
[log_image_file](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image_file**),
[log_depth_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_depth_image**),
[log_segmentation_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_segmentation_image**)
* Rust (primary): [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)
* Primary component: `tensor`
* Secondary components: `colorrgba`, `draw_order`

`colorrgba` is currently only supported for images,
i.e. tensors with 2 dimensions and an optional 3rd that can be interpreted as color channels.
Furthermore, only the spatial Space View is able to use the color component.

## Plots

### Scalar
* Python: [log_scalar](https://ref.rerun.io/docs/python/latest/common/plotting/#rerun.log_scalar)
* Rust (primary): [Scalar](https://docs.rs/rerun/latest/rerun/components/struct.Scalar.html)
* Primary component: `scalar`
* Secondary components: `scalar_plot_props`, `colorrgba`, `radius`, `label`

## Text

### Text Entry
* Python: [log_text_entry](https://ref.rerun.io/docs/python/latest/common/text/#rerun.log_text_entry)
* Rust (primary): [TextEntry](https://docs.rs/rerun/latest/rerun/components/struct.TextEntry.html)
* Primary component: `text_entry`
* Secondary components: `colorrgba`
