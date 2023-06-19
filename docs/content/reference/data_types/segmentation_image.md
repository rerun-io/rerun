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

code-example: segmentation-image-simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/93700bada34f617307f287f56119bf58da3100c9_segmentation_image_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/0fece156087f6e732fa46ee3335e3dcecf82f186_segmentation_image_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/1c44231b9a1f4f5bfdb5b4008438fd7e7cde6369_segmentation_image_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/f51606b8f75c80ed867d033fb34c0b29ced75068_segmentation_image_simple_1200w.png">
  <img src="https://static.rerun.io/5117d78838e9eee11d45732dfcf68cbed49896eb_segmentation_image_simple_full.png" alt="">
</picture>

