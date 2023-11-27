---
title: "SegmentationImage"
---

An image made up of integer class-ids.

The shape of the `TensorData` must be mappable to an `HxW` tensor.
Each pixel corresponds to a depth value in units specified by meter.

Leading and trailing unit-dimensions are ignored, so that
`1x640x480x1` is treated as a `640x480` image.

## Components

**Required**: [`TensorData`](../components/tensor_data.md)

**Optional**: [`DrawOrder`](../components/draw_order.md)

## Links
 * 🐍 [Python API docs for `SegmentationImage`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.SegmentationImage)
 * 🌊 [C++ API docs for `SegmentationImage`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1SegmentationImage.html?speculative-link)
 * 🦀 [Rust API docs for `SegmentationImage`](https://docs.rs/rerun/latest/rerun/archetypes/struct.SegmentationImage.html)

## Example

### Simple segmentation image

code-example: segmentation_image_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/1200w.png">
  <img src="https://static.rerun.io/segmentation_image_simple/eb49e0b8cb870c75a69e2a47a2d202e5353115f6/full.png" width="640">
</picture>
</center>

