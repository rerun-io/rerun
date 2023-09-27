---
title: Image
order: 20
---

`Image` represents a 2D raster image with various pixel format. They are a special case of 2D [Tensor](tensor.md) with an optional 3rd dimension when multiple color channels are used. Image with 1 (grayscale), 3 (RGB), or 4 (RGBA) channels are supported. Color channel maybe represented by any of the common scalar datatypes:

- `uint8`, `uint16`, `uint32`, `uint64`: Color channels in 0-`max_uint` sRGB gamma space, alpha in 0-`max_int` linear space.
- `float16`, `float32`, `float64`: Color channels in the 0.0-1.0 sRGB gamma space, alpha in 0.0-1.0 linear space.
- `int8`, `int16`, `int32`, `int64`: If all pixels are positive, they are interpreted as their unsigned counterparts. Otherwise, the image is normalized before display (the pixel with the lowest value is black and the pixel with the highest value is white).

The `colorrgba` component for the image applies a multiplicative tint.

## Components and APIs

Primary component: `tensor`

Secondary components: `colorrgba`, `draw_order`

Python APIs: [log_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image**), [log_image_file](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image_file**),

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)

## Simple Example

code-example: image_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png">
  <img src="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/full.png" alt="">
</picture>

## Advanced Example

code-example: image_advanced

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/image_advanced/aeee879303ccf36f9665646ab46242f188005752/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/image_advanced/aeee879303ccf36f9665646ab46242f188005752/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/image_advanced/aeee879303ccf36f9665646ab46242f188005752/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/image_advanced/aeee879303ccf36f9665646ab46242f188005752/1200w.png">
  <img src="https://static.rerun.io/image_advanced/aeee879303ccf36f9665646ab46242f188005752/full.png" alt="">
</picture>
