---
title: "TensorBuffer"
---

The underlying storage for a `Tensor`.

Tensor elements are stored in a contiguous buffer of a single type.

## Variants

* U8: list of `u8`
* U16: list of `u16`
* U32: list of `u32`
* U64: list of `u64`
* I8: list of `i8`
* I16: list of `i16`
* I32: list of `i32`
* I64: list of `i64`
* F16: list of `f16`
* F32: list of `f32`
* F64: list of `f64`
* JPEG: list of `u8`
* NV12: list of `u8`
* YUY2: list of `u8`

## Links
 * 🌊 [C++ API docs for `TensorBuffer`](https://ref.rerun.io/docs/cpp/stable/structrerun_1_1datatypes_1_1TensorBuffer.html)
 * 🐍 [Python API docs for `TensorBuffer`](https://ref.rerun.io/docs/python/stable/common/datatypes#rerun.datatypes.TensorBuffer)
 * 🦀 [Rust API docs for `TensorBuffer`](https://docs.rs/rerun/latest/rerun/datatypes/enum.TensorBuffer.html)


## Used by

* [`TensorData`](../datatypes/tensor_data.md)
