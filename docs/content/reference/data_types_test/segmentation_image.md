---
title: segmentation_image
order: 100
---

An image made up of integer class-ids

The shape of the `TensorData` must be mappable to an `HxW` tensor.
Each pixel corresponds to a depth value in units specified by meter.

Leading and trailing unit-dimensions are ignored, so that
`1x640x480x1` is treated as a `640x480` image.

## Components and APIs

Required:
* tensor_data

Optional:
* draw_order

## Examples

### segmentation_image_simple

code-example: segmentation_image_simple


