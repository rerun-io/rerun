---
title: Migrating from 0.17 to 0.18
order: 992
---

## ⚠️ Breaking changes
### [`DepthImage`](https://rerun.io/docs/reference/types/archetypes/depth_image) and [`SegmentationImage`](https://rerun.io/docs/reference/types/archetypes/segmentation_image)
The `DepthImage` and `SegmentationImage` archetypes used to be encoded as tensors, but now they are encoded as blobs of bytes with an [`ImageFormat`](https://rerun.io/docs/reference/types/components/image_format) consisting of a resolution and a datatype.
The resolution is now specified in `[width, height]` order.

The Python & Rust APIs are largely unchanged, but in particular C++ users need to be careful to use the correct shape order. Also, C++ constructors have changed and expect now either `rerun::Collection` or raw pointers as their first arguments respectively:

Before:
```cpp
rec.log("segmentation", rerun::SegmentationImage({HEIGHT, WIDTH}, data));
rec.log("depth", rerun::DepthImage({HEIGHT, WIDTH}, data).with_meter(10000.0));
```
After:
```cpp
rec.log("segmentation", rerun::SegmentationImage(data.data(), {WIDTH, HEIGHT}));
rec.log("depth", rerun::DepthImage(pixels.data(), {WIDTH, HEIGHT}).with_meter(10000.0));
```


### [`Image`](https://rerun.io/docs/reference/types/archetypes/image)
The `Image` and `SegmentationImage` archetypes used to be encoded as tensors, but now they are encoded as blobs of bytes with an [`ImageFormat`](https://rerun.io/docs/reference/types/components/image_format) consisting of a resolution and a datatype.
Special formats like `NV12` are specified by a `PixelFormat` enum which takes precedence over the datatype and color-model specified in the `ImageFormat`.
The resolution is now specified in `[width, height]` order.

#### Python
The `data` argument of the `Image()` constructor has been removed.
The first default parameter is now `image`, which can be a `numpy.ArrayLike`, which will also be used to extract the relevant metadata.
Alternatively images can also be constructed using a `bytes` argument, but the resolution and pixel format must be provided explicitly.

#### C++
Argument order has changed. Also, make sure to specify resolution in the corrected order:

Before:
```cpp
rec.log("image", rerun::Image({HEIGHT, WIDTH, 3}, data));
```
After:
```cpp
rec.log("image", rerun::Image(data, {WIDTH, HEIGHT}, datatypes::ColorModel::RGB));
```

The same can now also achieved with this utility:
```cpp
rec.log("image", rerun::Image::from_rgb24(data, {WIDTH, HEIGHT}));
```

### [`EncodedImage`](https://rerun.io/docs/reference/types/archetypes/encoded_image)
`EncodedImage` is our new archetype for logging an image file, e.g. a PNG or JPEG.

#### Python
`rr.ImageEncoded` is deprecated. Image files (JPEG, PNG, …) should instead be logged with [`EncodedImage`](https://rerun.io/docs/reference/types/archetypes/encoded_image),
and chroma-downsampled images (NV12/YUY2) are now logged with the new `Image` archetype:

Before:
```python
rr.log("NV12", rr.ImageEncoded(contents=nv12_bytes, format=rr.ImageFormat.NV12((height, width))))
```

After:
```python
rr.log("NV12", rr.Image(bytes=nv12_bytes, width=width, height=height, pixel_format=rr.PixelFormat.NV12))
```

#### Rust
* Removed `TensorBuffer::JPEG`
* Removed `TensorData::from_jpeg_bytes`
* Deprecated `Image::from_file_path` and `from_file_contents`

For all of these, use [`EncodedImage`](https://rerun.io/docs/reference/types/archetypes/encoded_image) instead.


### `mesh_material: Material` has been renamed to `albedo_factor: AlbedoFactor` [#6841](https://github.com/rerun-io/rerun/pull/6841)
The field `mesh_material` in `Mesh3D` is now named `albedo_factor` and wraps a `datatypes.Rgba32`.

When constructing a [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d):
* C++ & Rust: `.with_mesh_material(Material::from_albedo_factor(color))` ➡ `with_albedo_factor(color)`
* Python: `mesh_material=rr.Material(albedo_factor=color)` ➡ `albedo_factor=color`


### Overhaul of [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/Transform3D)
Previously, the transform component was represented as one of several variants (an Arrow union, `enum` in Rust) depending on how the transform was expressed, sometimes nested within.
(for instance, the `TranslationRotationScale3D` variant had internally several variants for rotation & scale).

Instead, there are now several components for translation/scale/rotation/matrices that can live side-by-side in the [3D transform archetype](https://rerun.io/docs/reference/types/archetypes/transform3d).

For this purpose `TranslationRotationScale3D` and `TranslationAndMat3x3` datatypes & components have been removed and split up into new components:
* [`Translation3D`](https://rerun.io/docs/reference/types/components/translation3d)
* [`TransformMat3x3`](https://rerun.io/docs/reference/types/components/transform_mat3x3)
* [`Scale3D`](https://rerun.io/docs/reference/types/components/scale3d)
* [`RotationAxisAngle`](https://rerun.io/docs/reference/types/components/rotation_axis_angle)
   * uses existing datatype with the same name
* [`RotationQuat`](https://rerun.io/docs/reference/types/components/rotation_quat)
   * uses existing `Quaternion` datatype
* [`TransformRelation`](https://rerun.io/docs/reference/types/components/transform_relation)
   * this replaces the previous `from_parent` bool
   * `from_parent` is still available in all SDK languages, but deprecated

All components are applied to the final transform in the opposite order they're listed in.
This means that if translation, rotation & scale are applied, then (just as in 0.17 and earlier), the object is first scaled, then rotated and then translated (from the point of view of the parent space).

When a `Transform3D` archetype is sent, _all_ components are written, even if you don't set them.
This means that if you first log a `Transform3D` with a `Translation3D` and then later another `Transform3D` with a `RotationQuat`, this will result in an entity that is only rotated.

Other changes in data representation:
* Scaling no longer distinguishes uniform and 3D scaling in its data representation, it is now always expressed as 3 floats with the same value. Helper functions are provided to build uniform scales.
* Angles (as used in `RotationAxisAngle`) are now always stored in radians, conversion functions for degrees are provided.

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

`rerun::Transform3D::IDENTITY` has been removed, use `rerun::Transform3D()` to start out with
an empty archetype instead that you can populate (e.g. `rerun::Transform3D().with_mat3x3(rerun::datatypes::Mat3x3::IDENTITY)`).


Scale is no longer an enum datatype but a component with a 3D vector:
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
rerun::Transform3D::clear().with_mat3x3(matrix).with_translation(translation)
```

Note that the order of the method calls does _not_ affect the order in which transformation is applied!

`Transform3D::clear` is named so, because whenever you send the `Transform3D` archetype, it will clear ALL of its components,
by sending an empty value for them.
This means logging a `Transform3D::from_rotation(…)` followed by a `Transform3D::from_translation(…)` will only result in the translation, as the later log call will clear the previous rotation.

### `OutOfTreeTransform3D` removed in favor of [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d)
[`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) fulfills an extended role:
It works more similar to the [`Transform3D`](https://rerun.io/docs/reference/types/archetypes/transform3d) archetype and is supported by all 3D spatial primitives.
Furthermore, it can be used for instancing 3D meshes and is used to represent the poses of boxes and ellipsoids/spheres.

#### Python
Asset3D previously had a `transform` argument, now you have to send either a `InstancePoses3D` or a `Transform3D` on the same entity:
Before:
```python
rr.log("world/asset", rr.Asset3D(
        path=path,
        transform=rr.OutOfTreeTransform3DBatch(
            rr.TranslationRotationScale3D(translation=center, scale=scale)
        )
    ))
```
After:
```python
rr.log("world/asset", rr.Asset3D(path=path), rr.InstancePoses3D(translation=center, scale=scale))
```

#### C++
Asset3D previously had a `transform` field, now you have to send either a `InstancePoses3D` or a `Transform3D` on the same entity:
Before:
```cpp
rec.log("world/asset", rerun::Asset3D::from_file(path).value_or_throw()
                    .with_transform(rerun::OutOfTreeTransform3D(translation))
);
```
After:
```cpp
rec.log("world/asset",
    rerun::Asset3D::from_file(path).value_or_throw(),
    rerun::InstancePoses3D().with_translations(translation)
);
```

#### Rust
Asset3D previously had a `transform` field, now you have to send either a `InstancePoses3D` or a `Transform3D` on the same entity:
Before:
```rust
rec.log("world/asset", &rerun::Asset3D::from_file(path)?
        .with_transform(rerun::OutOfTreeTransform3D::from(rerun::TranslationRotationScale3D(translation)))
)?;
```
After:
```rust
rec.log("world/asset", &rerun::Asset3D::from_file(path)?)?;
rec.log("world/asset", &rerun::InstancePoses3D::default().with_translations([translation]))?;
```

### [`Boxes3D`](https://rerun.io/docs/reference/types/archetypes/boxes3d) changes

`centers` is now a `PoseTranslation3D` instead of a [`Position3D`](https://rerun.io/docs/reference/types/components/position3d) component.
The main difference in behavior is that this means it overlaps with the newly introduced [`InstancePoses3D`](https://rerun.io/docs/reference/types/archetypes/instance_poses3d) archetype.

`rotation` was removed in favor of `rotation_axis_angles` and `quaternions` which are
`PoseRotationAxisAngle` and `PoseRotationQuat` components.
Consequently, instead of using `with_rotations` (C++/Rust) or `rotation=` (Python) you'll need to use `with_quaternions`/`quaternions=`  or `with_rotation_axis_angles`/`rotation_axis_angles=` respectively.
