<!--[metadata]
title = "Raw mesh"
description = "Log a 3D scene as raw `Mesh3D` data or directly as a prepacked `Asset3D`."
thumbnail = "https://static.rerun.io/raw-mesh/7731418dda47e15dbfc0f9a2c32673909071cb40/480w.png"
thumbnail_dimensions = [480, 480]
-->

This example demonstrates two ways to log the same GLB scene: converting it to raw [`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d) data or sending the original file as an [`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).
The example uses `Mesh3D` by default and switches to `Asset3D` when you pass `--asset3d`.

## Choosing between `Mesh3D` and `Asset3D`

Use `Asset3D` for assets in a supported format such as GLB, glTF, OBJ, or STL when you don't care about the details of the encoded data.
Rerun stores the file and loads its meshes, embedded materials, and transform hierarchy in the viewer.
Prefer self-contained assets such as GLB because referenced files are not included automatically.

Use `Mesh3D` for generated meshes, unsupported formats, or explicit control over vertex data.
Logging primitives and transforms as separate entities lets you query and update them independently, but requires your application to parse and convert the source data.

## Logging with `Mesh3D`

By default, the example parses the GLB file and logs each mesh primitive and transform separately.

```rust
let mesh: rerun::Mesh3D = primitive.into();
rec.log(format!("{}/{}", node.name, i), &mesh)?;
```

## Logging with `Asset3D`

With `--asset3d`, the scene file is stored in the recording as a single `Asset3D`.

```rust
rec.log(
    "world/asset",
    &rerun::Asset3D::from_file_path(scene_path)?,
)?;
```

## Run the code

Run the raw `Mesh3D` conversion:

```bash
cargo run -p raw_mesh
```

Pass `--asset3d` to log the selected scene as a prepacked asset:

```bash
cargo run -p raw_mesh -- --asset3d
```

The flag works with both `--scene` and `--scene-path`.
Run `cargo run -p raw_mesh -- --help` for more options.

<picture>
  <img src="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/raw_mesh/d5d008b9f1b53753a86efe2580443a9265070b77/1200w.png">
</picture>
