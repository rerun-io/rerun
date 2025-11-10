<!--[metadata]
title = "Raw mesh"
tags = ["Mesh"]
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
channel = "release"
-->

Demonstrates how to construct and log raw 3D mesh data (so-called "triangle soups") programmatically from scratch.

This example shows how to create mesh geometry by manually defining vertices, normals, colors, and texture coordinates for various geometric primitives, each demonstrating different features of the [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d) archetype.

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

## Geometric Primitives

The example generates several geometric primitives, each showcasing different `Mesh3D` features:

### Cube (per-vertex colors)
A cube where each face has a different color, demonstrating how to use `vertex_colors` for per-vertex coloring.

### Pyramid (UV texture)
A four-sided pyramid with UV texture coordinates and a procedurally generated checkerboard texture, demonstrating `vertex_texcoords` and `albedo_texture`.

### Sphere (smooth shading)
A UV sphere with vertex normals for smooth shading, demonstrating `vertex_normals` and `albedo_factor`.

### Icosahedron (flat shading)
A 20-sided regular polyhedron rendered without vertex normals, resulting in flat-shaded faces.

## Logging and visualizing with Rerun

The visualizations in this example were created with the following Rerun code:

### Cube with vertex colors
```python
rr.log(
    "world/cube",
    rr.Mesh3D(
        vertex_positions=cube["vertex_positions"],
        vertex_colors=cube["vertex_colors"],
        triangle_indices=cube["triangle_indices"],
    ),
)
```

### Pyramid with texture
```python
rr.log(
    "world/pyramid",
    rr.Mesh3D(
        vertex_positions=pyramid["vertex_positions"],
        vertex_texcoords=pyramid["vertex_texcoords"],
        albedo_texture=texture,
        triangle_indices=pyramid["triangle_indices"],
    ),
)
```

### Sphere with smooth shading
```python
rr.log(
    "world/sphere",
    rr.Mesh3D(
        vertex_positions=sphere["vertex_positions"],
        vertex_normals=sphere["vertex_normals"],
        albedo_factor=np.array([100, 150, 255, 255], dtype=np.uint8),
        triangle_indices=sphere["triangle_indices"],
    ),
)
```

### Icosahedron with flat shading
```python
rr.log(
    "world/icosahedron",
    rr.Mesh3D(
        vertex_positions=icosahedron["vertex_positions"],
        albedo_factor=np.array([255, 180, 100, 255], dtype=np.uint8),
        triangle_indices=icosahedron["triangle_indices"],
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
