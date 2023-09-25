---
title: image
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
* tensor_data

Optional:
* draw_order

## Examples

### image_simple

code-example: image_simple


