---
title: Loggable Data Types
order: 2
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
* [Point2D](data_types/point2d.md)
* [Point3D](data_types/point3d.md)

## Image & Tensor **Archetypes**
* [Image](data_types/image.md)
* [DepthImage](data_types/depth_image.md)
* [SegmentationImage](data_types/segmentation_image.md)
* [Tensor](data_types/tensor.md)

## Plot **Archetypes**
* [Scalar](data_types/scalar.md)

## Text **Archetypes**
* [TextEntry](data_types/text_entry.md)
