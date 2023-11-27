---
title: "Image"
---

A monochrome or color image.

The shape of the `TensorData` must be mappable to:
- A `HxW` tensor, treated as a grayscale image.
- A `HxWx3` tensor, treated as an RGB image.
- A `HxWx4` tensor, treated as an RGBA image.

Leading and trailing unit-dimensions are ignored, so that
`1x640x480x3x1` is treated as a `640x480x3` RGB image.

## Components

**Required**: [`TensorData`](../components/tensor_data.md)

**Optional**: [`DrawOrder`](../components/draw_order.md)

## Links
 * 🌊 [C++ API docs for `Image`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Image.html?speculative-link)
 * 🐍 [Python API docs for `Image`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Image)
 * 🦀 [Rust API docs for `Image`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Image.html)

## Example

### image_simple

code-example: image_simple

<center>
<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
  <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png" width="640">
</picture>
</center>

