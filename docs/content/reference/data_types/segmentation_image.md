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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/eb1c4dfd9d8900b7bb649b29806426024dc327f9_segmentation_image_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/1a67fa53ca1bdf77b614ab5740d8199ca85b09d3_segmentation_image_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/4af05a520c4eb0cb52ccc116ccd3022b92d1590b_segmentation_image_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/50b630c63a8ec0f8ec2b5c8b3ca4974b17db93fd_segmentation_image_simple_1200w.png">
  <img src="https://static.rerun.io/eb49e0b8cb870c75a69e2a47a2d202e5353115f6_segmentation_image_simple_full.png" alt="">
</picture>


