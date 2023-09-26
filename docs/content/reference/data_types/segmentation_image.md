---
title: SegmentationImage
order: 22
---

Segmentation images are 2D images containing segmentation information. They are 2D tensors with a single channel of type `uint8` or `uint16` containing the class ID for each pixel. A corresponding color and/or label can be mapped to class IDs using an [annotation context](annotation_context.md).


## Components and APIs
Primary component: `tensor`

Secondary components: `draw_order`

Python APIs: [log_segmentation_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_segmentation_image**)

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)


## Simple example

code-example: segmentation_image_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1200w.png">
  <img src="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/full.png" alt="">
</picture>


