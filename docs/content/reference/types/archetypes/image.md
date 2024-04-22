---
title: "Image"
---

A monochrome or color image.

The order of dimensions in the underlying `TensorData` follows the typical
row-major, interleaved-pixel image format. Additionally, Rerun orders the
`TensorDimension`s within the shape description from outer-most to inner-most.

As such, the shape of the `TensorData` must be mappable to:
- A `HxW` tensor, treated as a grayscale image.
- A `HxWx3` tensor, treated as an RGB image.
- A `HxWx4` tensor, treated as an RGBA image.

Leading and trailing unit-dimensions are ignored, so that
`1x480x640x3x1` is treated as a `480x640x3` RGB image.

Rerun also supports compressed image encoded as JPEG, N12, and YUY2.
Using these formats can save a lot of bandwidth and memory.

## Components

**Required**: [`TensorData`](../components/tensor_data.md)

**Optional**: [`DrawOrder`](../components/draw_order.md)

## Links
 * 🌊 [C++ API docs for `Image`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1archetypes_1_1Image.html)
 * 🐍 [Python API docs for `Image`](https://ref.rerun.io/docs/python/stable/common/archetypes#rerun.archetypes.Image)
 * 🦀 [Rust API docs for `Image`](https://docs.rs/rerun/latest/rerun/archetypes/struct.Image.html)

## Example

### image_simple

snippet: image_simple

<picture data-inline-viewer="snippets/image_simple">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
  <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png">
</picture>

