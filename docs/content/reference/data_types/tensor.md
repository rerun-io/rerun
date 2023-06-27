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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/8e8d7f4137c598de8158effa4b82c5ef4d93ee23_tensor_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/1a8fe9f89e523b0060a2f6adea08385e1d8bef21_tensor_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/d1bb2d7dc290f54aecafec5f767308123ed18242_tensor_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/baa09cc431692ad8f0cab21b9184c6dbf4a62ff5_tensor_simple_1200w.png">
  <img src="https://static.rerun.io/1aead2554496737e9267a5ab5220dbc89da851ee_tensor_simple_full.png" alt="">
</picture>

## 1-D Tensor Example

code-example: tensor_one_dim

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/5beafb5eeac8f7799a699441478fb48b97e6a90b_tensor_one_dim_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/cb2268cb986396a17106d4bc7bc4a33984321077_tensor_one_dim_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/8434209ad97f30c5c9f139a114a4f7a5a30098af_tensor_one_dim_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/c5930d7965b86b4454030e492b6791a0f322b457_tensor_one_dim_1200w.png">
  <img src="https://static.rerun.io/cbf24b466fe9d9639777aefb34f1a00c3f30d7ab_tensor_one_dim_full.png" alt="">
</picture>
