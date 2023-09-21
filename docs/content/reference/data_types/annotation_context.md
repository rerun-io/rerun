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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/9bb6f96009ac9b4991ba5b6ceeb954a9204bf656_annotation_context_rects_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/c5453e6fb1a1a44396d1cc3ac80c63c67c9fba56_annotation_context_rects_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/d3aed394aaad90f8ada96e54a4e66f16992d1817_annotation_context_rects_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/2ff32dfd45d15f35f7d0947c26445d4113fe6d03_annotation_context_rects_1200w.png">
  <img src="https://static.rerun.io/9b446c36011ed30fce7dc6ed03d5fd9557460f70_annotation_context_rects_full.png" alt="">
</picture>


## Segmentation example

code-example: annotation_context_segmentation

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/97d397dd0cb5d094e2227aef22785f45bcae4a18_annotation_context_segmentation_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/bf72a7c47d5b56f37741ae101cb3f992ffc54b8c_annotation_context_segmentation_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/aca2e6946c586cceeeb9d33c0d8da867e111d5b7_annotation_context_segmentation_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/c77eef272ba23d58b6a2cbf980ca88a42a17207d_annotation_context_segmentation_1200w.png">
  <img src="https://static.rerun.io/0e21c0a04e456fec41d16b0deaa12c00cddf2d9b_annotation_context_segmentation_full.png" alt="">
</picture>


## Connections example

code-example: annotation_context_connections

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/7fc503e76810264da70fc18806eadf987ebd703e_annotation_context_connections_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/50ec6309ce791d9f85153d00a737031b1632448d_annotation_context_connections_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cf9998a0ccccee42aacc1de0773ea8801a129cdd_annotation_context_connections_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/53f483421562f8d4bbb8c7e695058069ce1ab00c_annotation_context_connections_1200w.png">
  <img src="https://static.rerun.io/4a8422bc154699c5334f574ff01b55c5cd1748e3_annotation_context_connections_full.png" alt="">
</picture>
