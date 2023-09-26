---
title: AnnotationContext
order: 50
---

Annotation Contexts are metadata providing information to the Rerun viewer on how to interpret and display other entities. Currently, three types of annotations are supported:

- Labels and/or colors for [Rect2D](rect2d.md) and [Box3D](box3d.md) entities. These are mapped to the rectangle and box entities via their `class_id` components. Commonly used for object detection.
- Labels and/or colors for [segmentation images](segmentation_image.md). These are mapped to the images' pixel values, which are interpreted as `class_id`s.
- Labels, colors, and/or connections for [Point2D](point2d.md) and [Position3D](point3d.md) entities. These are mapped to the point entities via their `class_id` and `keypoint_id` components. Commonly used for keypoint or pose detection.

See the [Annotation Context](../../concepts/annotation-context.md) concept page for more information.


## Components and APIs
Primary component: `annotation_context`

Python APIs: [log_annotation_context](https://ref.rerun.io/docs/python/latest/common/annotations/#rerun.log_annotation_context)

Rust API: [AnnotationContext](https://docs.rs/rerun/latest/rerun/components/struct.AnnotationContext.html)

## Rectangles example

code-example: annotation_context_rects

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png">
  <img src="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/full.png" alt="">
</picture>


## Segmentation example

code-example: annotation_context_segmentation

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/1200w.png">
  <img src="https://static.rerun.io/annotation_context_segmentation/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b/full.png" alt="">
</picture>


## Connections example

code-example: annotation_context_connections

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1200w.png">
  <img src="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/full.png" alt="">
</picture>
