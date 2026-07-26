<!--[metadata]
title = "Raw mesh"
description = "Log raw 3D mesh data (\"triangle soups\") with simple material properties and a transform hierarchy via `Mesh3D`."
tags = ["Mesh"]
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
include_in_manifest = true
-->

Demonstrates logging of procedurally generated raw 3D mesh data (so-called "triangle soups") with simple material properties and a transform hierarchy.
For prepacked mesh files such as GLTF, GLB, OBJ, and STL, use [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) instead.

<picture data-inline-viewer="examples/raw_mesh">
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

## Used Rerun types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d)

## Background
Raw 3D mesh data refers to the basic geometric representation of a three-dimensional object, typically composed of interconnected triangles.
These triangles collectively form the surface of the object, defining its shape and structure in a digital environment.
Rerun was employed to visualize and manage this raw mesh data, along with its associated simple material properties and transform hierarchy.


## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### 3D mesh data
The raw 3D mesh data are logged as [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) objects.
This example creates one mesh as a non-indexed triangle soup with per-face normals and colors, and another as indexed triangles with a material color.

```python
rr.log(
    "world/base",
    rr.Mesh3D(
        vertex_positions=positions,
        vertex_normals=normals,
        vertex_colors=colors,
    ),
    static=True,
)
```
Through Rerun's [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetype, essential details are captured to ensure precise positioning and orientation of meshes within the 3D scene.
```python
rr.log(
    "world/base/arm",
    rr.Transform3D(translation=(0.0, 0.0, 0.9)),
    static=True,
)
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
To experiment with the provided example, execute the main Python script:
```bash
python -m raw_mesh # run the example
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python -m raw_mesh --help
```
