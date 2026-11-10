---
title: Control mesh appearance
order: 270
description: Mesh colors, textures, transparency, normals, and lighting for Mesh3D and Asset3D, and why a mesh looks flat or unlit
---

This page explains how [`Mesh3D`](../../reference/types/archetypes/mesh3d.md) and [`Asset3D`](../../reference/types/archetypes/asset3d.md) are shaded, and which settings you can change.

## Lighting

The 3D view uses simple, fixed lighting: two directional lights and some ambient light.
You cannot add lights, move them, or turn on shadows.
There are no material properties, such as roughness, metallic surfaces, or specular highlights.
See [missing features](../../development/roadmap.md#missing-features).

## Why does my mesh look flat or unlit?

Lighting needs vertex normals.

- `Mesh3D` without `vertex_normals` is drawn _unlit_: every triangle gets its plain color, so the shape is hard to see.
  Pass `vertex_normals` to get shading.
- `Asset3D` computes flat normals for meshes in the file that have none.
  To draw an asset unlit, give it all-zero vertex normals.

To compute smooth vertex normals for a `Mesh3D` yourself, you can use a library such as [trimesh](https://trimesh.org/):

```python
import trimesh

mesh = trimesh.load("model.obj", force="mesh")
rr.log(
    "mesh",
    rr.Mesh3D(
        vertex_positions=mesh.vertices,
        triangle_indices=mesh.faces,
        vertex_normals=mesh.vertex_normals,
    ),
)
```

## Colors and textures

- `vertex_colors`: one color per vertex.
- `albedo_factor`: one color for the whole mesh.
  It is multiplied with the vertex colors and the texture.
- `albedo_texture` together with `vertex_texcoords`: a texture image.

See the [`Mesh3D` reference](../../reference/types/archetypes/mesh3d.md) for an example of a textured mesh.
For `Asset3D`, colors and textures come from the file, and you can tint the whole asset with `albedo_factor`.

## Transparency

The only way to make a mesh transparent is the alpha channel of `albedo_factor`:

```python
rr.log("mesh", rr.Mesh3D(vertex_positions=positions, triangle_indices=triangles, albedo_factor=[255, 255, 255, 128]))
```

The alpha channel of `vertex_colors` and of the texture is ignored.

## Front and back faces

By default, both sides of each triangle are drawn.
Set `face_rendering` to `"Front"` or `"Back"` to draw only one side.
Front faces have their vertices in counter-clockwise order.

```python
rr.Mesh3D(vertex_positions=positions, triangle_indices=triangles, face_rendering="Front")
```
