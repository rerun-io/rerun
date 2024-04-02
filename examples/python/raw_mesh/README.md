<!--[metadata]
title = "Raw Mesh"
tags = ["mesh"]
description = "Demonstrates logging of raw 3D mesh data with simple material properties."
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
-->

<picture data-inline-viewer="examples/raw_mesh">
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>

Demonstrates logging of raw 3D mesh data (so-called "triangle soups") with simple material properties and their transform hierarchy.

# Used Rerun Types
[`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d), [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d)

# Background
Raw 3D mesh data refers to the basic geometric representation of a three-dimensional object, typically composed of interconnected triangles. 
These triangles collectively form the surface of the object, defining its shape and structure in a digital environment. 
Rerun was employed to visualize and manage this raw mesh data, along with its associated simple material properties and transform hierarchy.


# Logging and Visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

## 3D Mesh Data
The raw 3D mesh data are logged as [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) objects, and includes details about vertex positions, colors, normals, texture coordinates, material properties, and face indices for an accurate reconstruction and visualization.

```python
rr.log(
    path,
    rr.Mesh3D(
        vertex_positions=mesh.vertices,
        vertex_colors=vertex_colors,
        vertex_normals=mesh.vertex_normals,
        vertex_texcoords=vertex_texcoords,
        albedo_texture=albedo_texture,
        indices=mesh.faces,
        mesh_material=mesh_material,
    ),
)
```
Through Rerun's [`Transform3D`](https://www.rerun.io/docs/reference/types/archetypes/transform3d) archetype, essential details are captured to ensure precise positioning and orientation of meshes within the 3D scene.
```python
rr.log(
    path,
    rr.Transform3D(
        translation=trimesh.transformations.translation_from_matrix(world_from_mesh),
        mat3x3=world_from_mesh[0:3, 0:3],
    ),
)
```


# Run the code
To run this example, make sure you have the Rerun repository checked out and the latest SDK installed:
```bash
# Setup 
pip install --upgrade rerun-sdk  # install the latest Rerun SDK
git clone git@github.com:rerun-io/rerun.git  # Clone the repository
cd rerun
git checkout latest  # Check out the commit matching the latest SDK release
```
Install the necessary libraries specified in the requirements file:
```bash
pip install -r examples/python/raw_mesh/requirements.txt
```
To experiment with the provided example, simply execute the main Python script:
```bash
python examples/python/raw_mesh/main.py # run the example
```
You can specify scene:
```bash
python examples/python/objectron/main.py --scene {lantern,avocado,buggy,brain_stem}
```
If you wish to customize it, explore additional features, or save it use the CLI with the `--help` option for guidance:
```bash
python examples/python/raw_mesh/main.py --help 
```
