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

code-example: depth-image-simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/8dc31747abadd7e07ba9baabc66a9356236b203d_depth_image_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/43cb845d37fb43b3342a37802535b562512768dc_depth_image_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/4e8e822f0ccc7f87e6f7f1e290068c827f8e9a07_depth_image_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/8360413f0c01450cd260a3306c1cd2c95ed46744_depth_image_simple_1200w.png">
  <img src="https://static.rerun.io/e22f6b0fa18146faf5bdff44cd93f215a9888a40_depth_image_simple_full.png" alt="">
</picture>
