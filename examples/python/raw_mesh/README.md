<!--[metadata]
title = "Raw mesh"
description = "Log a 3D scene as raw `Mesh3D` data or directly as a prepacked `Asset3D`."
tags = ["Mesh"]
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
-->

Demonstrates two ways to log the same GLB scene: converting it to raw [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d) data or sending the original file as an [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).
The example uses `Mesh3D` by default and switches to `Asset3D` when you pass `--asset3d`.

## Choosing between `Mesh3D` and `Asset3D`

Use `Asset3D` for assets in a supported format such as GLB, glTF, OBJ, or STL when you don't care about the details of the encoded data.
Rerun stores the file and loads its meshes, embedded materials, and transform hierarchy in the viewer.
Prefer self-contained assets such as GLB because referenced files are not included automatically.

Use `Mesh3D` for generated meshes, unsupported formats, or explicit control over vertex data.
Logging primitives and transforms as separate entities lets you query and update them independently, but requires your application to parse and convert the source data.

<picture data-inline-viewer="examples/raw_mesh">
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

## Used Rerun types
[`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d), [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d)

## Background
Raw 3D mesh data refers to the basic geometric representation of a three-dimensional object, typically composed of interconnected triangles.
These triangles collectively form the surface of the object, defining its shape and structure in a digital environment.
The default code path uses `trimesh` to parse a GLB file, then logs its raw mesh data, simple material properties, and transform hierarchy to Rerun.


## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Raw 3D mesh data

The raw 3D mesh data is logged as [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) objects.
It includes vertex positions, colors, normals, texture coordinates, material properties, and face indices.

```python
rr.log(
    path,
    rr.Mesh3D(
        vertex_positions=mesh.vertices,
        vertex_colors=vertex_colors,
        vertex_normals=mesh.vertex_normals,
        vertex_texcoords=vertex_texcoords,
        albedo_texture=albedo_texture,
        triangle_indices=mesh.faces,
        albedo_factor=albedo_factor,
    ),
)
```

The [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetype preserves the position and orientation of each mesh in the scene hierarchy.

```python
rr.log(
    path,
    rr.Transform3D(
        translation=trimesh.transformations.translation_from_matrix(world_from_mesh),
        mat3x3=world_from_mesh[0:3, 0:3],
    ),
)
```

### Prepacked 3D asset

With `--asset3d`, the scene file is stored in the recording as a single [`Asset3D`](https://www.rerun.io/docs/reference/types/archetypes/asset3d).

```python
rr.log("world", rr.ViewCoordinates.RUB, static=True)
rr.log("world/asset", rr.Asset3D(path=scene_path))
```

## Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -e examples/python/raw_mesh
```
To experiment with the provided example, simply execute the main Python script:
```bash
python -m raw_mesh # run the example
```
You can specify the scene:
```bash
python -m raw_mesh --scene {lantern,avocado,buggy,brain_stem}
```
Pass `--asset3d` to log the selected scene as a prepacked asset instead of converting it to raw mesh data:
```bash
python -m raw_mesh --asset3d
```
The flag works with both `--scene` and `--scene-path`.
For more options:
```bash
python -m raw_mesh --help
```
