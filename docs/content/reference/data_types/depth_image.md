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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/5ed6b5ce014c0fbbc70dc4241c117b10610e1ce7_depth_image_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/8786f135fc56814d968002249cec00f74b93947c_depth_image_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/fd5130f688b8dcb8b4d7a39cae373b94e72f0dd6_depth_image_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/a978c1f8dbb3d7d86f0895f649999f779a02c12e_depth_image_simple_1200w.png">
  <img src="https://static.rerun.io/f78674bdae0eb25786c6173307693c5338f38b87_depth_image_simple_full.png" alt="">
</picture>

