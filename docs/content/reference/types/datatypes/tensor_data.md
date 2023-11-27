---
title: "TensorData"
---

A multi-dimensional `Tensor` of data.

The number of dimensions and their respective lengths is specified by the `shape` field.
The dimensions are ordered from outermost to innermost. For example, in the common case of
a 2D RGB Image, the shape would be `[height, width, channel]`.

These dimensions are combined with an index to look up values from the `buffer` field,
which stores a contiguous array of typed values.

## Fields

* shape: [`TensorDimension`](../datatypes/tensor_dimension.md)
* buffer: [`TensorBuffer`](../datatypes/tensor_buffer.md)

## Links
 * 🌊 [C++ API docs for `TensorData`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1datatypes_1_1TensorData.html?speculative-link)
 * 🐍 [Python API docs for `TensorData`](https://ref.rerun.io/docs/python/stable/common/datatypes#rerun.datatypes.TensorData)
 * 🦀 [Rust API docs for `TensorData`](https://docs.rs/rerun/latest/rerun/datatypes/struct.TensorData.html)


## Used by

* [`TensorData`](../components/tensor_data.md)
