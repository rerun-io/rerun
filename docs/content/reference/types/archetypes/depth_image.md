---
title: "DepthImage"
---

A depth image.

The shape of the `TensorData` must be mappable to an `HxW` tensor.
Each pixel corresponds to a depth value in units specified by `meter`.

## Components

**Required**: [`TensorData`](../components/tensor_data.md)

**Optional**: [`DepthMeter`](../components/depth_meter.md), [`DrawOrder`](../components/draw_order.md)

## Links
 * 🌊 [C++ API docs for `DepthImage`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1DepthImage.html)
 * 🐍 [Python API docs for `DepthImage`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.DepthImage)
 * 🦀 [Rust API docs for `DepthImage`](https://docs.rs/rerun/latest/rerun/archetypes/struct.DepthImage.html)

## Examples

### Simple example

snippet: depth_image_simple

<picture data-inline-viewer="snippets/depth_image_simple">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/1200w.png">
  <img src="https://static.rerun.io/depth_image_simple/9598554977873ace2577bddd79184ac120ceb0b0/full.png">
</picture>

### Depth to 3D example

snippet: depth_image_3d

<picture data-inline-viewer="snippets/depth_image_3d">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/1200w.png">
  <img src="https://static.rerun.io/depth_image_3d/f78674bdae0eb25786c6173307693c5338f38b87/full.png">
</picture>

