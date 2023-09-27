---
title: Image
order: 100
---

A monochrome or color image.

The shape of the `TensorData` must be mappable to:
- A `HxW` tensor, treated as a grayscale image.
- A `HxWx3` tensor, treated as an RGB image.
- A `HxWx4` tensor, treated as an RGBA image.

Leading and trailing unit-dimensions are ignored, so that
`1x640x480x3x1` is treated as a `640x480x3` RGB image.

## Components and APIs

Required:
* `tensor_data`

Optional:
* `draw_order`

## Examples

### image_simple

code-example: image_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
  <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png" alt="screenshot of image_simple example">
</picture>

