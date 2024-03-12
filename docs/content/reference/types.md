---
title: Types
order: 2
---

Rerun comes with built-in support for a number of different types that can be logged via the Python and Rust Logging
APIs and then visualized in the [Viewer](viewer.md).

The top-level types are called [**archetypes**](types/archetypes.md) to differentiate them from the lower-level
[**data types**](types/datatypes.md) that make up the individual [**components**](types/components.md).
For more information on the relationship between **archetypes** and **components**, check out the concept page
on [Entities and Components](../concepts/entity-component.md).

## Spatial **Archetypes**
The spatial archetypes represent 2D and 3D spatial data. These types have some notion of a coordinate system and
generally support spatial transformations. These types can be visualized by 2D and 3D space views.

To visualize a 2D entity in a 3D space view it must be under a pinhole transformation.
To visualize a 3D entity in a 2D space view, the 2D's origin must be at a pinhole transform and all 3D objects are above it.

* [Arrow3D](types/archetypes/arrows3d.md)
* [Asset](types/archetypes/asset3d.md)
* [Box2D](types/archetypes/boxes2d.md)
* [Box3D](types/archetypes/boxes3d.md)
* [LineStrip2D](types/archetypes/line_strips2d.md)
* [LineStrip3D](types/archetypes/line_strips3d.md)
* [Mesh](types/archetypes/mesh3d.md)
* [Point2D](types/archetypes/points2d.md)
* [Point3D](types/archetypes/points3d.md)

### Spatial transformations

* [Transform3D](types/archetypes/transform3d.md)
* [Pinhole](types/archetypes/pinhole.md)
* [DisconnectedSpace](types/archetypes/disconnected_space.md): disconnect an entity path from its parent.


## Image & Tensor **Archetypes**
Image and tensor archetypes all build on top of a common tensor component. The tensor component is a multi-dimensional
generic container for arrays of data. Images are restricted to tensors of rank 2 or rank 3; these can be viewed in the
`Spatial` space view. Generic tensors of greater rank can only be viewed in the specialized `Tensor` space view.
* [Image](types/archetypes/image.md)
* [DepthImage](types/archetypes/depth_image.md)
* [SegmentationImage](types/archetypes/segmentation_image.md)
* [Tensor](types/archetypes/tensor.md)

## Time Series **Archetypes**
* [Scalar](types/archetypes/scalar.md): a single scalar / metric value.
* [SeriesPoint](types/archetypes/series_point.md): define the style properties for a point series in a chart.
* [SeriesLine](types/archetypes/series_line.md): define the style properties for a line series in a chart.
* [TimeSeriesScalar (deprecated)](types/archetypes/time_series_scalar.md): a single scalar / metric value as well as styling options. Can be viewed in the `TimeSeries` space view.

## Other **Archetypes**
* [AnnotationContext](types/archetypes/annotation_context.md): not viewed directly, but provides classes, labels, and connectivity information for other entities.
* [BarChart](types/archetypes/bar_chart.md): data displayed in a `BarChart` space view.
* [Clear](types/archetypes/clear.md): clear all components of an entity.
* [TextDocument](types/archetypes/text_document.md): text displayed in a `TextDocument` space view.
* [TextLog](types/archetypes/text_log.md): a log entry in a `TextLog` space view.
* [ViewCoordinates](types/archetypes/view_coordinates.md): determines how we interpret the coordinate system of an entity/space.

