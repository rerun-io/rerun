<!--[metadata]
title = "Raw mesh"
tags = ["Mesh"]
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
-->

Demonstrates how to construct and log raw 3D mesh data (so-called "triangle soups") programmatically from scratch.

This example shows how to create mesh geometry by manually defining vertices, normals, colors, and texture coordinates, then demonstrating different material properties of the [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d) archetype by reusing the same base geometry.

If you want to log existing mesh files (like GLTF, OBJ, STL, etc.), use the [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) archetype instead.

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
Raw 3D mesh data refers to the basic geometric representation of a three-dimensional object, typically composed of interconnected triangles (vertices, edges, and faces). This example demonstrates how to construct such data programmatically without loading external files.

## Mesh Material Properties

The example generates a single sphere geometry and reuses it with different material parameters to showcase various features of the `Mesh3D` archetype:

### Vertex Colors
The base sphere with per-vertex colors that create a gradient based on vertex position.

### Albedo Factor
A solid color applied to the entire mesh using the `albedo_factor` parameter (pink in this example).

### Albedo Texture
UV texture coordinates with a procedurally generated checkerboard texture demonstrating how to apply images to meshes.

### Vertex Normals
Vertex normals for smooth shading, showing how surface orientation affects lighting.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Vertex colors
Raw 3D mesh data is logged as [`Mesh3D`](https://www.rerun.io/docs/reference/types/archetypes/mesh3d) objects with per-vertex colors:

```python
rr.log(
    "world/sphere/vertex_colors",
    rr.Mesh3D(
        vertex_positions=sphere_data["vertex_positions"],
        vertex_colors=sphere_data["vertex_colors"],
        triangle_indices=sphere_data["triangle_indices"],
    ),
)
```

### Albedo factor
Apply a solid color to the entire mesh:

```python
rr.log(
    "world/sphere/albedo_factor",
    rr.Mesh3D(
        vertex_positions=sphere_data["vertex_positions"],
        albedo_factor=np.array([255, 100, 150, 255], dtype=np.uint8),
        triangle_indices=sphere_data["triangle_indices"],
    ),
)
```

### Textured meshes
For meshes with textures, provide UV coordinates and a texture:

```python
rr.log(
    "world/sphere/albedo_texture",
    rr.Mesh3D(
        vertex_positions=sphere_data["vertex_positions"],
        vertex_texcoords=sphere_data["vertex_texcoords"],
        albedo_texture=texture,
        triangle_indices=sphere_data["triangle_indices"],
    ),
)
```

### Smooth shading with normals
For smooth shading, provide vertex normals:

```python
rr.log(
    "world/sphere/vertex_normals",
    rr.Mesh3D(
        vertex_positions=sphere_data["vertex_positions"],
        vertex_normals=sphere_data["vertex_normals"],
        albedo_factor=np.array([100, 150, 255, 255], dtype=np.uint8),
        triangle_indices=sphere_data["triangle_indices"],
    ),
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
To experiment with the provided example, simply execute the main Python script:
```bash
python -m raw_mesh # run the example
```
You can customize the sphere subdivisions for more or less detail:
```bash
python -m raw_mesh --sphere-subdivisions 64
```
If you wish to explore additional features or save it, use the CLI with the `--help` option for guidance:
```bash
python -m raw_mesh --help
```
