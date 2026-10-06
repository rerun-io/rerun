---
title: Raw Bayer images
hidden: true
type: feature
---

### Raw Bayer images

`Image` supports raw Bayer images through new `PixelFormat` variants for the RGGB, BGGR, GBRG and GRBG patterns, at 8 bits per pixel.
The viewer interprets Bayer samples as linear color values and demosaics them on the GPU.

<picture>
  <img src="https://static.rerun.io/mosaic-droid/328303e534f582719293ca5a4b8f130d828dfb33/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/mosaic-droid/328303e534f582719293ca5a4b8f130d828dfb33/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/mosaic-droid/328303e534f582719293ca5a4b8f130d828dfb33/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mosaic-droid/328303e534f582719293ca5a4b8f130d828dfb33/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mosaic-droid/328303e534f582719293ca5a4b8f130d828dfb33/1200w.png">
</picture>

See [`PixelFormat`](../reference/types/encodings/pixel_format.md).
