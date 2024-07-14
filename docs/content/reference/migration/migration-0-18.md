---
title: Migrating from 0.17 to 0.18 (unreleased)
order: 180
---

NOTE! Rerun 0.18 has not yet been released


## ⚠️ Breaking changes
### `ImageEncoded`
`ImageEncoded` is our new archetype for logging an image file, e.g. a PNG or JPEG.

#### Python
In python we already had a `ImageEncoded` class, but this has now been replaced with the new archetype.

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
