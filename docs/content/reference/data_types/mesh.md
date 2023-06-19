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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/50a0201e08bfc843d8a544db2e0ed5ccb65a1fde_mesh_simple_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/0660d59216f467be310507c6f1d93880d9cddd10_mesh_simple_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/0113f054b20d365c14922cfdad2140a2f7e29045_mesh_simple_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/cbb51e91b7aa65fa4774b32031e93d4718b8da77_mesh_simple_1200w.png">
  <img src="https://static.rerun.io/c13648317223585abe28df8bcaa8c933587558b6_mesh_simple_full.png" alt="">
</picture>
