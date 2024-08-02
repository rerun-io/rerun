---
title: Migrating from 0.17 to 0.18 (unreleased)
order: 180
---

NOTE! Rerun 0.18 has not yet been released


## ⚠️ Breaking changes
### [`DepthImage`](https://rerun.io/docs/reference/types/archetypes/depth_image) and [`SegmentationImage`](https://rerun.io/docs/reference/types/archetypes/segmentation_image)
The `DepthImage` and `SegmentationImage` archetypes used to be encoded as a tensor, but now it is encoded as a blob of bytes, a resolution, and a datatype.
The constructs have changed to now expect the shape in `[width, height]` order.


### [`Image`](https://rerun.io/docs/reference/types/archetypes/image)
* `Image.compress` has been replaced by `ImageEncoded.compress`
* `Image` now support chroma-downsampled images

`Image(…)` now require a _color_model_ argument, e.g. "RGB" or "L"
* Before: `rr.Image(image_rgb)`
* Now: `rr.Image(image_rgb, "RGB")`


### [`ImageEncoded`](https://rerun.io/docs/reference/types/archetypes/image_encoded?speculative-link)
`ImageEncoded` is our new archetype for logging an image file, e.g. a PNG or JPEG.

#### Python
In Python we already had a `ImageEncoded` class, but this has now been replaced with the new archetype.

* Python: `NV12/YUY2` are now logged with the new `Image`:


```py
rr.log(
    "my_image",
    rr.Image(
        bytes=…,
        width=…,
        height=…,
        pixel_format=rr.PixelFormat.Nv12,
    ),
)
```

* `ImageEncoded`:s `format` parameter has been replaced with `media_type` (MIME)
    * `ImageFormat` is now only for `NV12/YUY2`

### Rust
* Removed `TensorBuffer::JPEG`
* Removed `TensorData::from_jpeg_bytes`
* Deprecated `Image::from_file_path` and `from_file_contents`

For all of these, use `ImageEncoded` instead.


### `mesh_material: Material` has been renamed to `albedo_factor: AlbedoFactor` [#6841](https://github.com/rerun-io/rerun/pull/6841)
The field `mesh_material` in `Mesh3D` is now named `albedo_factor` and wraps a `datatypes.Rgba32`.

When constructing a [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d):
* C++ & Rust: `.with_mesh_material(Material::from_albedo_factor(color))` -> `with_albedo_factor(color)`
* Python: `mesh_material=rr.Material(albedo_factor=color)` -> `albedo_factor=color`


### Overhaul of [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/Transform3D)

In order to simplify the Arrow schema (which determines how data is stored and retrieved) wide reaching changes have been made to the Transform3D API.
Previously, the transform component was represented as one of several variants (an Arrow union, `enum` in Rust) depending on how the transform was expressed, sometimes nested within.
(for instance, the `TranslationRotationScale3D` variant had internally several variants for rotation & scale).

Instead, there are now several components for translation/scale/rotation/matrices that can live side-by-side in the [3D transform archetype](https://rerun.io/docs/reference/types/archetypes/transform3d).

For this purpose `TranslationRotationScale3D` and `TranslationAndMat3x3` datatypes & components have been removed and split up into new components:
* [`Translation3D`](https://rerun.io/docs/reference/types/components/translation3d#speculative-link)
* [`TransformMat3x3`](https://rerun.io/docs/reference/types/components/transform_mat3x3#speculative-link)
* [`Scale3D`](https://rerun.io/docs/reference/types/components/scale3d#speculative-link)
* [`RotationAxisAngle`](https://rerun.io/docs/reference/types/components/rotation_axis_angle#speculative-link)
   * uses existing datatype with the same name
* [`RotationQuat`](https://rerun.io/docs/reference/types/components/rotation_quat#speculative-link)
   * uses existing `Quaternion` datatype
* [`TransformRelation`](https://rerun.io/docs/reference/types/components/transform_relation#speculative-link)
   * this replaces the previous `from_parent` bool
   * `from_parent` is still available in all SDK languages, but deprecated

All components are applied to the final transform in the opposite order they're listed in. E.g. if both a 4x4 matrix and a translation is set, the entity is first translated and then transformed with the matrix.
If translation, rotation & scale are applied, then (just as in prior versions), from the point of view of the parent space the object is first scaled, then rotated and then translated.

Other changes in data representation:
* Scaling no longer distinguishes uniform and 3D scaling in its data representation, it is now always expressed as 3 floats with the same value. Helper functions are provided to build uniform scales.
* Angles (as used in `RotationAxisAngle`) are now always stored in radians, conversion functions for degrees are provided.
Scaling no longer distinguishes uniform and 3D scaling in its data representation. Uniform scaling is now always expressed as 3 floats with the same value.

`OutOfTreeTransform3D` got removed. Instead, there is now a new [`LeafTransforms3D`](https://rerun.io/docs/reference/types/archetypes/leaf_transform3d#speculative-link). archetype which fulfills the same role, but works more similar to the `Transform3D` archetype and is supported by all 3D spatial primitives.


#### Python

The `Transform3D` archetype no longer has a `transform` argument. Use one of the other arguments instead.

Before:
```python
rr.log("myentity", rr.Transform3D(rr.TranslationRotationScale3D(translation=Vec3D([1, 2, 3]), from_parent=True)))
```
After:
```python
rr.log("myentity", rr.Transform3D(translation=Vec3D([1, 2, 3]), relation=rr.TransformRelation.ChildFromParent))
```

Asset3D previously had a `transform` argument, now you have to log either a `LeafTransform3D` or a `Transform3D` on the same entity:
Before:
```python
rr.log("world/mesh", rr.Asset3D(
        path=path,
        transform=rr.OutOfTreeTransform3DBatch(
            rr.TranslationRotationScale3D(translation=center, scale=scale)
        )
    ))
```
After:
```python
rr.log("world/mesh", rr.Asset3D(path=path))
rr.log("world/mesh", rr.LeafTransform3D(translation=center, scale=scale))
```

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


Scale is no longer an enum datatype but a component with a 3D vec:
Before:
```cpp
auto scale_uniform = rerun::Scale3D::Uniform(2.0);
auto scale_y = rerun::Scale3D::ThreeD([1.0, 2.0, 1.0]);
```
After:
```cpp
auto scale_uniform = rerun::Scale3D::uniform(2.0);
auto scale_y = rerun::Scale3D::from([1.0, 2.0, 1.0]);
```

Asset3D previously had a `transform` field, now you have to log either a `LeafTransform3D` or a `Transform3D` on the same entity:
Before:
```cpp
rec.log("world/asset", rerun::Asset3D::from_file(path).value_or_throw()
                    .with_transform(rerun::OutOfTreeTransform3D(translation))
);
```
After:
```cpp
rec.log("world/asset", rerun::Asset3D::from_file(path).value_or_throw());
rec.log("world/mesh", &rerun::archetypes::LeafTransform3D().with_translations(translation));
```

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
            rerun::Quaternion::from_xyzw(transform.r),
            transform.s,
        )
    }
}
```

Since all aspects of the transform archetypes are now granular, they can be chained with `with_` functions:
```rust
rerun::Transform3D::default().with_mat3x3(matrix).with_translation(translation)
```
Note that the order of the method calls does _not_ affect the order in which transformation is applied!

`rerun::Transform3D::IDENTITY` has been removed, sue `rerun::Transform3D::default()` to start out with
an empty archetype instead that you can populate (e.g. `rerun::Transform3D::default().with_mat3x3(rerun::datatypes::Mat3x3::IDENTITY)`).


Asset3D previously had a `transform` field, now you have to log either a `LeafTransform3D` or a `Transform3D` on the same entity:
Before:
```rust
rec.log("world/mesh", &rerun::Asset3D::from_file(path)?
        .with_transform(rerun::OutOfTreeTransform3D::from(rerun::TranslationRotationScale3D(translation)))
)?;
```
After:
```rust
rec.log("world/mesh", &rerun::Asset3D::from_file(path)?)?;
rec.log("world/mesh", &rerun::LeafTransform3D::default().with_translations([translation]))?;
```

### [`Boxes3D`](https://rerun.io/docs/reference/types/archetypes/boxes3d) changes

`centers` is now a [`LeafTranslation3D`](https://rerun.io/docs/reference/types/components/leaf_translation3d#speculative-link) instead of a [`Position3D`](https://rerun.io/docs/reference/types/components/position3d) component.
The main difference in behavior is that this means it overlaps with the newly introduced [`LeafTransforms3D`](https://rerun.io/docs/reference/types/archetypes/leaf_transform3d#speculative-link) archetype.

`rotation` was removed in favor of `rotation_axis_angles` and `quaternions` which are
[`LeafRotationAxisAngle`](https://rerun.io/docs/reference/types/components/leaf_rotation_axis_angle#speculative-link) and `LeafRotationQuat`(https://rerun.io/docs/reference/types/components/leaf_rotation_quat#speculative-link) components.
Consequently, instead of using `with_rotations` (C++/Rust) or `rotation=` (Python) you'll need to use `with_quaternions`/`quaternions=`  or `with_rotation_axis_angles`/`rotation_axis_angles=` respectively.
