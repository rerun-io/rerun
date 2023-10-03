---
title: Loggable Data Types
order: 2
---

Rerun comes with built-in support for a number of different types that can be logged via the Python and Rust Logging
APIs and then visualized in the [Viewer](viewer.md).

The top-level types are called [**archetypes**](data_types/archetypes.md) to differentiate them from the lower-level
[**data types**](data_types/datatypes.md) that make up the individual [**components**](data_types/components.md).
For more information on the relationship between **archetypes** and **components**, check out the concept page
on [Entities and Components](../concepts/entity-component.md).

In [Python](https://ref.rerun.io) every **archetype** is typically backed by one or more function calls. In
contrast, the [Rust API](https://docs.rs/rerun/) works by building up entities of a given archetype explicitly by
assembling the required components.

## Spatial **Archetypes**
The spatial archetypes represent 2d and 3d spatial data. These types have some notion of a coordinate system and
generally support spatial transformations. All of these types can be visualized by the `Spatial` space view.
* [Arrow3D](data_types/archetypes/arrows3d.md)
* [Asset](data_types/archetypes/asset3d.md)
* [Box2D](data_types/archetypes/boxes2d.md)
* [Box3D](data_types/archetypes/boxes3d.md)
* [LineStrip2D](data_types/archetypes/line_strips2d.md)
* [LineStrip3D](data_types/archetypes/line_strips3d.md)
* [Mesh](data_types/archetypes/mesh3d.md)
* [Point2D](data_types/archetypes/points2d.md)
* [Point3D](data_types/archetypes/points3d.md)
* [Transform3D](data_types/archetypes/transform3d.md)
* [Pinhole](data_types/archetypes/pinhole.md)

## Image & Tensor **Archetypes**
Image and tensor archetypes all build on top of a common tensor component. The tensor component is a multi-dimensional
generic container for arrays of data. Images are restricted to tensors of rank 2 or rank 3; these can be viewed in the
`Spatial` space view. Generic tensors of greater rank can only be viewed in the specialized `Tensor` space view.
* [Image](data_types/archetypes/image.md)
* [DepthImage](data_types/archetypes/depth_image.md)
* [SegmentationImage](data_types/archetypes/segmentation_image.md)
* [Tensor](data_types/archetypes/tensor.md)

## Other **Archetypes**
* [AnnotationContext](data_types/archetypes/annotation_context.md): not viewed directly, but provides classes, labels, and connectivity information for other entities.
* [BarChart](data_types/archetypes/bar_chart.md): data displayed in a `BarChart` space view.
* [Clear](data_types/archetypes/clear.md): clear all components of an entity.
* [DisconnectedSpace](data_types/archetypes/disconnected_space.md): disconnect an entity path from its parent.
* [TextDocument](data_types/archetypes/text_document.md): text displayed in a `TextDocument` space view.
* [TextLog](data_types/archetypes/text_log.md): a log entry in a `TextLog` space view.
* [TimeSeriesScalar](data_types/archetypes/time_series_scalar.md): a single scalar / metric value. Can be viewed in the `TimeSeries` space view.
* [ViewCoordinates](data_types/archetypes/view_coordinates.md): determines how we interpret the coordinate system of an entity/space.

