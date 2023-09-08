---
title: Loggable Data Types
order: 2
---

Rerun comes with built-in support for a number of different types that can be logged via the Python and Rust Logging
APIs and then visualized in the [Viewer](viewer.md).

The top-level types are called **archetypes** to differentiate them from the lower-level **data types** that make up the
individual components.  For more information on the relationship between **archetypes** and **components**, check out
the concept page on [Entities and Components](../concepts/entity-component.md).

In [Python](https://ref.rerun.io) every **archetype** is typically backed by one or more function calls. In
contrast, the [Rust API](https://docs.rs/rerun/) works by building up entities of a given archetype explicitly by
assembling the required components.

## Spatial **Archetypes**
The spatial archetypes represent 2d and 3d spatial data. These types have some notion of a coordinate system and
generally support spatial transformations. All of these types can be visualized by the `Spatial` space view.
* [Arrow3D](data_types/arrow3d.md)
* [Rect2D](data_types/rect2d.md)
* [Box3D](data_types/box3d.md)
* [Linestrip2D](data_types/linestrip2d.md)
* [Linestrip3D](data_types/linestrip3d.md)
* [Mesh](data_types/mesh.md)
* [Point2D](data_types/point2d.md)
* [Point3D](data_types/point3d.md)
* [Transform3D](data_types/transform3d.md)
* [Pinhole](data_types/pinhole.md)

## Image & Tensor **Archetypes**
Image and tensor archetypes all build on top of a common tensor component. The tensor component is a multi-dimensional
generic container for arrays of data. Images are restricted to tensors of rank 2 or rank 3; these can be viewed in the
`Spatial` space view. Generic tensors of greater rank can only be viewed in the specialized `Tensor` space view.
* [Image](data_types/image.md)
* [DepthImage](data_types/depth_image.md)
* [SegmentationImage](data_types/segmentation_image.md)
* [Tensor](data_types/tensor.md)

## Other **Archetypes**
* [Scalar](data_types/scalar.md): a single scalar / metric value. Can be viewed in the `TimeSeries` space view.
* [AnnotationContext](data_types/annotation_context.md): not viewed directly, but provides classes, labels, and connectivity information for other entities.
