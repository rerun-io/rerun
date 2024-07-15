---
title: Migrating from 0.17 to 0.18 (unreleased)
order: 180
---

NOTE! Rerun 0.18 has not yet been released


## ⚠️ Breaking changes
### `ImageEncoded`
`ImageEncoded` is our new archetype for logging an image file, e.g. a PNG or JPEG.

#### Python
In Python we already had a `ImageEncoded` class, but this has now been replaced with the new archetype.

* Python: `NV12/YUY2` are now logged with the new `ImageChromaDownsampled`
* `ImageEncoded`:s `format` parameter has been replaced with `media_type` (MIME)
    * `ImageFormat` is now only for `NV12/YUY2`

### Rust
* Removed `TensorBuffer::JPEG`
* Removed `TensorData::from_jpeg_bytes`
* Deprecated `Image::from_file_path` and `from_file_contents`

For all of these, use `ImageEncoded` instead.


### `mesh_material: Material` has been renamed to `albedo_factor: AlbedoFactor` [#6841](https://github.com/rerun-io/rerun/pull/6841)
The field `mesh_material` in `Mesh3D` is now named `albedo_factor` and wraps a `datatypes.Rgba32`.

When constructing a `Mesh3D`:
* C++ & Rust: `.with_mesh_material(Material::from_albedo_factor(color))` -> `with_albedo_factor(color)`
* Python: `mesh_material=rr.Material(albedo_factor=color)` -> `albedo_factor=color`


### Overhaul of Transform3D

In order to simplify the Arrow schema (which determines how data is stored and retrieved) wide reaching changes have been made to the Transform3D API.
Previously, the transform component was represented as one of several variants (an Arrow union, `enum` in Rust) depending on how the transform was expressed, sometimes nested within.
(for instance, the `TranslationRotationScale3D` variant had internally several variants for rotation & scale).

Instead, there are now several components for translation/scale/rotation/matrices that can live side-by-side in the [3D transform archetype](https://rerun.io/docs/reference/types/archetypes/transform3d).

For this purpose `TranslationRotationScale3D` and `TranslationAndMat3x3` datatypes & components have been removed and split up into new components:
* [`Translation3D`](https://rerun.io/docs/reference/types/components/translation3d#speculative-link)
* [`TransformMat3x3`](https://rerun.io/docs/reference/types/components/transform_mat3x3#speculative-link)
* TODO(andreas): More!

All components are applied to the final transform in order. E.g. if both a 4x4 matrix and a translation is set, the entity is first transformed with the matrix and then translated.

Scaling no longer distinguishes uniform and 3D scaling in its data representation. Uniform scaling is now always expressed as 3 floats with the same value.

TODO(andreas): Write about OutOfTreeTransform changes and how `Transform3D` has now arrays of components.


#### Python

The `Transform3D` archetype no longer has a `transform` argument. Use one of the other arguments instead.
TODO(andreas): Not true as of writing. but should be true at the time or release!

Before:
```python
rr.log("myentity", rr.Transform3D(rr.TranslationRotationScale3D(translation=Vec3D([1, 2, 3]), from_parent=True)))
```
After:
```python
rr.log("myentity", rr.Transform3D(translation=Vec3D([1, 2, 3]), from_parent=True))
```


TODO(andreas): code example


TODO(andreas): Talk about OutOfTreeTransform
TODO(andreas): … and Asset3D specifically


#### C++

Most of the previous constructors of `rerun::Transform3D` archetype are still present. However,
most of them expect now concrete components which oftentimes makes automatic type conversion fail.

It's recommended to use the new explicit factory methods instead. For example:

Before:
```cpp
rec.log("myentity", rerun::Transform3D({1.0f, 2.0f, 3.0f}));
```
After:
```cpp
rec.log("myentity", rerun::Transform3D::from_translation({1.0f, 2.0f, 3.0f}));
```

Since all aspects of the transform archetypes are now granular, they can be chained with `with_` functions:
```cpp
rerun::Transform3D().with_mat3x3(matrix).with_translation(translation)
```
Note that the order of the method calls does _not_ affect the order in which transformation is applied!

`rerun::Transform3D::IDENTITY` has been removed, sue `rerun::Transform3D()` to start out with
an empty archetype instead that you can populate (e.g. `rerun::Transform3D().with_mat3x3(rerun::datatypes::Mat3x3::IDENTITY)`).


Scale is no longer an enum datatype but a component with a 3d vec:
Before:
```rust
let scale_uniform = rerun::Scale3D::Uniform(2.0);
let scale_y = rerun::Scale3D::ThreeD([1.0, 2.0, 1.0]);
```
After:
```rust
let scale_uniform = rerun::Scale3D::uniform(2.0);
let scale_y = rerun::Scale3D::from([1.0, 2.0, 1.0]);
```

TODO(andreas): Talk about OutOfTreeTransform
TODO(andreas): … and Asset3D specifically

#### Rust
`rerun::archetypes::Transform3D` no longer has a `new`, use other factory methods instead, e.g. `from_translation_rotation_scale` or `from_mat3x3`

Before:
```rust
rec.log("myentity", &rerun::archetypes::Transform3D::new(translation))?;
```
After:
```rust
rec.log("myentity", &rerun::archetypes::Transform3D::from_translation(translation))?;
```

Instead of building the now removed `Transform3D` component, you often can use the archetype directly:

Before:
```rust
impl From<GltfTransform> for rerun::components::Transform3D {
    fn from(transform: GltfTransform) -> Self {
        rerun::components::Transform3D::from_translation_rotation_scale(
            transform.t,
            rerun::datatypes::Quaternion::from_xyzw(transform.r),
            transform.s,
        )
    }
}
```
After:
```rust
impl From<GltfTransform> for rerun::Transform3D {
    fn from(transform: GltfTransform) -> Self {
        rerun::Transform3D::from_translation_rotation_scale(
            transform.t,
            rerun::datatypes::Quaternion::from_xyzw(transform.r),
            transform.s,
        )
    }
}
```
TODO(andreas): Quaternion in above snippet is likely to change as well.

Since all aspects of the transform archetypes are now granular, they can be chained with `with_` functions:
```rust
rerun::Transform3D::default().with_mat3x3(matrix).with_translation(translation)
```
Note that the order of the method calls does _not_ affect the order in which transformation is applied!

`rerun::Transform3D::IDENTITY` has been removed, sue `rerun::Transform3D::default()` to start out with
an empty archetype instead that you can populate (e.g. `rerun::Transform3D::default().with_mat3x3(rerun::datatypes::Mat3x3::IDENTITY)`).

TODO(andreas): Talk about OutOfTreeTransform
TODO(andreas): … and Asset3D specifically
