---
title: "TensorData"
---

A multi-dimensional `Tensor` of data.

The number of dimensions and their respective lengths is specified by the `shape` field.
The dimensions are ordered from outermost to innermost. For example, in the common case of
a 2D RGB Image, the shape would be `[height, width, channel]`.

These dimensions are combined with an index to look up values from the `buffer` field,
which stores a contiguous array of typed values.

Note that the buffer may be encoded in a compressed format such as `jpeg` or
in a format with downsampled chroma, such as NV12 or YUY2.
For file formats, the shape is used as a hint, for chroma downsampled format
the shape has to be the shape of the decoded image.

## Fields

* data: [`TensorData`](../datatypes/tensor_data.md)

## Links
 * 🌊 [C++ API docs for `TensorData`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1components_1_1TensorData.html)
 * 🐍 [Python API docs for `TensorData`](https://ref.rerun.io/docs/python/stable/common/components#rerun.components.TensorData)
 * 🦀 [Rust API docs for `TensorData`](https://docs.rs/rerun/latest/rerun/components/struct.TensorData.html)


## Used by

* [`BarChart`](../archetypes/bar_chart.md)
* [`DepthImage`](../archetypes/depth_image.md)
* [`Image`](../archetypes/image.md)
* [`SegmentationImage`](../archetypes/segmentation_image.md)
* [`Tensor`](../archetypes/tensor.md)
