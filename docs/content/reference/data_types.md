---
title: Loggable Data Types
order: 3
---

Rerun comes with built-in support for a number of different types that can be logged via the Python and Rust Logging
APIs and then visualized in the [Viewer](../viewer.md).

The top-level types are called **archetypes** to differentiate them from the lower-level **data types** that make up the
individual components.  For more information on the relationship between **archetypes** and **components**, check out
the concept page on [Entities and Components](../concepts/entity-component.md).

In [Python](https://ref.rerun.io) every **archetype** is typically backed by one or more function calls. In
contrast, the [Rust API](https://docs.rs/rerun/) works by builting up entities of a given archetype explicitly by
assembling the required components.

## Spatial **Archetypes**
The spatial archetypes represent 2d and 3d spatial data. These types have some notion of a coordinate system and
generally support spatial transformations.
* [Arrow3D](data_types/arrow3d.md)
* [Rectangle2D](data_types/rectangle3d.md)
* [Box3D](data_types/box3d.md)
* [Line2D](data_types/line2d.md)
* [Line3D](data_types/line3d.md)
* [Mesh](data_types/mesh.md)
* [Points2D](data_types/point2d.md)
* [Points3D](data_types/point3d.md)

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
