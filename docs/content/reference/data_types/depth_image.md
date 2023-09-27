---
title: DepthImage
order: 21
---

A depth image is a 2D image containing depth information. It is a 2D tensor with a single channel of type `uint16`, `float32`, or `float64`. It can be displayed in a 3D viewer when combined with a [pinhole camera](pinhole.md).

## Components and APIs

Primary component: `tensor`

Secondary components: `draw_order`

Python APIs: [log_depth_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_depth_image**),

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)


## Simple example

code-example: depth_image_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/1200w.png">
  <img src="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/full.png" alt="">
</picture>

## Depth to 3D example

code-example: depth_image_3d

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/1200w.png">
  <img src="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/full.png" alt="">
</picture>
