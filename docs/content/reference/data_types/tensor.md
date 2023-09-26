---
title: Tensor
order: 23
---

Tensors are N-dimensional data matrix with homogeneous type. Supported types are:

- Unsigned integers: `uint8`, `uint16`, `uint32`, `uint64`
- Signed integers: `uint8`, `uint16`, `uint32`, `uint64`
- Floating point numbers: `float16`, `float32`, `float64`

Note: 1-D tensors are visualized as bar charts.

## Components and APIs
Primary component: `tensor`

Python APIs: [log_tensor](https://ref.rerun.io/docs/python/latest/common/tensors/#rerun.log_tensor),

Rust API: [Tensor](https://docs.rs/rerun/latest/rerun/components/struct.Tensor.html)

## Simple Example

code-example: tensor_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/1200w.png">
  <img src="https://static.rerun.io/tensor_simple/1aead2554496737e9267a5ab5220dbc89da851ee/full.png" alt="">
</picture>

## 1-D Tensor Example

code-example: tensor_one_dim

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/tensor_one_dim/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/tensor_one_dim/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/tensor_one_dim/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/tensor_one_dim/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab/1200w.png">
  <img src="https://static.rerun.io/tensor_one_dim/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab/full.png" alt="">
</picture>
