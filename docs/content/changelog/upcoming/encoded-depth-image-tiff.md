---
title: TIFF support for EncodedDepthImage
hidden: true
type: feature
---

### TIFF support for `EncodedDepthImage`

`EncodedDepthImage` now accepts TIFF blobs (`image/tiff`) next to PNG and RVL.
The viewer decodes single channel TIFF with `U8`, `U16`, or `F32` samples on demand, so compressed depth stays small in the recording.

[`EncodedDepthImage` reference](../reference/types/archetypes/encoded_depth_image.md)
