---
title: Archetypes
order: 1
---

Archetypes represent a bundle of components.

## Spatial **Archetypes**
The spatial archetypes represent 2d and 3d spatial data. These types have some notion of a coordinate system and
generally support spatial transformations. All of these types can be visualized by the `Spatial` space view.
* [Arrow3D](archetypes/arrows3d.md)
* [Asset](archetypes/asset3d.md)
* [Box2D](archetypes/boxes2d.md)
* [Box3D](archetypes/boxes3d.md)
* [LineStrip2D](archetypes/line_strips2d.md)
* [LineStrip3D](archetypes/line_strips3d.md)
* [Mesh](archetypes/mesh3d.md)
* [Point2D](archetypes/points2d.md)
* [Point3D](archetypes/points3d.md)
* [Transform3D](archetypes/transform3d.md)
* [Pinhole](archetypes/pinhole.md)

## Image & Tensor **Archetypes**
Image and tensor archetypes all build on top of a common tensor component. The tensor component is a multi-dimensional
generic container for arrays of data. Images are restricted to tensors of rank 2 or rank 3; these can be viewed in the
`Spatial` space view. Generic tensors of greater rank can only be viewed in the specialized `Tensor` space view.
* [Image](archetypes/image.md)
* [DepthImage](archetypes/depth_image.md)
* [SegmentationImage](archetypes/segmentation_image.md)
* [Tensor](archetypes/tensor.md)

## Other **Archetypes**
* [AnnotationContext](archetypes/annotation_context.md): not viewed directly, but provides classes, labels, and connectivity information for other entities.
* [BarChart](archetypes/bar_chart.md): data displayed in a `BarChart` space view.
* [Clear](archetypes/clear.md): clear all components of an entity.
* [DisconnectedSpace](darchetypes/isconnected_space.md): disconnect an entity path from its parent.
* [TextDocument](archetypes/text_document.md): text displayed in a `TextDocument` space view.
* [TextLog](archetypes/text_log.md): a log entry in a `TextLog` space view.
* [TimeSeriesScalar](archetypes/time_series_scalar.md): a single scalar / metric value. Can be viewed in the `TimeSeries` space view.
* [ViewCoordinates](archetypes/view_coordinates.md): determines how we interpret the coordinate system of an entity/space.
