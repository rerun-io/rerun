---
title: Image
order: 20
---

`Image` represents a 2D raster image with various pixel format. They are a special case of 2D [Tensor](tensor.md) with an optional 3rd dimension when multiple color channels are used. Image with 1 (grayscale), 3 (RGB), or 4 (RGBA) channels are supported. Color channel maybe represented by any of the common scalar datatypes:

- `uint8`, `uint16`, `uint32`, `uint64`: color channels in 0-`max_uint` sRGB gamma space, alpha in 0-`max_int` linear space
- `float16`, `float32`, `float64`: color channels in the 0.0-1.0 sRGB gamma space, alpha in 0.0-1.0 linear space
- `int8`, `int16`, `int32`, `int64`: signed integers are cast into their unsigned counterpart without clipping


## Components and APIs

Primary component: `tensor`

Secondary components: `colorrgba`, `draw_order`

Note: `colorrgba` is currently only supported for images (i.e. 2D tensor with optional 3rd dimension). Furthermore, only the spatial Space View is able to use the color component.


Python APIs: [log_image](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image**), [log_image_file](https://ref.rerun.io/docs/python/latest/common/images/#rerun.log_image_file**),

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)

## Simple Example

code-example: image_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/39c48e5a87eeb62641c544e2604c99029192a297_image_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/fae9b2fc9da05e51261349ac6128635d85ae4bbb_image_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/4f0e279ee9b9712e2a8f4186606961d95a456347_image_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/f1e70cd3caec0979612491dcd9966ad781402780_image_simple_1200w.png">
  <img src="https://static.rerun.io/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4_image_simple_full.png" alt="">
</picture>

## Advanced Example

code-example: image_advanced

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/ccaeba024ee48b211d5bed9c4ee311530a1170ae_image_advanced_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/e71c397c545ecb6e2c1afef1e69aaf1b53ab241c_image_advanced_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/a9da6c281c77902e1eb10d74df81c15ad9f33c07_image_advanced_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/459241f37112c4a14057f8cfbc43b5eae48b0bd5_image_advanced_1200w.png">
  <img src="https://static.rerun.io/aeee879303ccf36f9665646ab46242f188005752_image_advanced_full.png" alt="">
</picture>
