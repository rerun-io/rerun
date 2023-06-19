---
title: Mesh
order: 6
---
`Mesh` represents a 3D mesh. It is defined by specifying its vertex positions, and optionally indices, normals,
albedo factor, and vertex-colors. `Mesh` entities will be drawn as part of the 3D Spatial SpaceView.

## Components and APIs
Primary component: `mesh3d`

Secondary components: `colorrgba`

Python APIs: [log_mesh](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_mesh), [log_meshes](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_meshes), [log_mesh_file](https://ref.rerun.io/docs/python/latest/common/spatial_primitives/#rerun.log_mesh_file)

Rust API: [Mesh3D](https://docs.rs/rerun/latest/rerun/components/enum.Mesh3D.html)

## Simple Examples

code-example: mesh_simple

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/dc83f393e523ade480e5b0e0b6d851c8ecbf4947_mesh_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/39270146f57757cf06ac1c9210b17fe1bfd5e25d_mesh_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/89ba9ace1950039fd9475c821e39e2bce16f0d45_mesh_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/0efa20c5627e4f113d4ceda3b93ad6a41c344cd3_mesh_simple_1200w.png">
  <img style="width: 75%;" src="https://static.rerun.io/7232cde57a4e68ec87a319ae3638a048731a29c9_mesh_simple_full.png" alt="">
</picture>
