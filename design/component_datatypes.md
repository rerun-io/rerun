# 2023-03-15 Component type conversions
Status: proposal

## Summary
Each _Component_ can be represented by many different _Datatypes_.

Every piece of data is associated with a `(component datatype)` tuple.

In the store we can have multiple columns of datatypes for each component.

Examples of `(component datatype)`:
```
(point2     [f16; 2])
(point3     [f32; 3])
(label      utf8)
(transform  mat4_f32)
(box2       box2_min_max_f32)
(box2       box2_min_size_u32)
(tensor     tensor_dense_v1)
(tensor     jpeg)
```

Both components and datatypes have namespaced names.

## Converters
We have a plugin-system for converting between from one `(comp, datatype)` to another.

The converters can match any `(comp, datatype)` pattern to any other, including wildcards.
For instance, `(*, vec3f16) -> (*, vec3f32)` and `(point2, vec2f32) -> (point3, vec3f32)`.

The plugin may specify if it should be on-the-fly, memoized, or write-back.

## Advantages

We can use this for:
  * compression (`Jpeg -> Tensor`)
  * different representation (`Mat4_f32` or `(Vec3_f32, Quat_XYZW_f32)`)
  * save space (store scalar as `u16`, or `f32`, or `f64`, …)
  * versioning (`Tensor_v2 -> Tensor_v3`)

### Open questions:
`(Transform, Jpeg)` or `(Jpeg, [u8])` ?

#### What naming convention for datatypes?
* Rust style: `[u8]`, `[f32; 3]` etc ?
* `quat_xyzw_f32` or `xyzw_f32` ?
