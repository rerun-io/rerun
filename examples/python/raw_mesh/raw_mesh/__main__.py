#!/usr/bin/env python3
"""
Shows how to log a 3D scene as raw mesh data or as a prepacked asset.

By default, the example parses the scene and logs its geometry and transform hierarchy with
[`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d).
Pass `--asset3d` to log the original file directly with
[`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import cast

import numpy as np
import trimesh

import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

from .download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded

DESCRIPTION = """
# 3D meshes
This example can log a 3D scene as explicit `Mesh3D` data or as a prepacked `Asset3D`.

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/raw_mesh).
"""


def load_scene(path: Path) -> trimesh.Scene:
    print(f"loading scene {path}…")
    mesh = trimesh.load(path, force="scene")
    return cast("trimesh.Scene", mesh)


# NOTE: The scene hierarchy will look different compared to the Rust example, as this is using the
# trimesh hierarchy, not the raw glTF hierarchy.
def log_scene(scene: trimesh.Scene, node: str, path: str | None = None) -> None:
    path = path + "/" + node if path else node

    parent = scene.graph.transforms.parents.get(node)
    children = scene.graph.transforms.children.get(node)

    node_data = scene.graph.get(frame_to=node, frame_from=parent)
    if node_data:
        # Log the transform between this node and its direct parent (if it has one!).
        if parent:
            # TODO(#3559): We should support 4x4 matrices directly
            world_from_mesh = node_data[0]
            rr.log(
                path,
                rr.Transform3D(
                    translation=trimesh.transformations.translation_from_matrix(world_from_mesh),
                    mat3x3=world_from_mesh[0:3, 0:3],
                ),
            )

        # Log this node's mesh, if it has one.
        mesh = cast("trimesh.Trimesh", scene.geometry.get(node_data[1]))
        if mesh is not None:
            vertex_colors = None
            vertex_texcoords = None
            albedo_factor = None
            albedo_texture = None

            try:
                vertex_texcoords = mesh.visual.uv  # type: ignore[union-attr]
                # trimesh uses the OpenGL convention for UV coordinates, so we need to flip the V coordinate
                # since Rerun uses the Vulkan/Metal/DX12/WebGPU convention.
                vertex_texcoords[:, 1] = 1.0 - vertex_texcoords[:, 1]
            except Exception:
                pass

            try:
                albedo_texture = mesh.visual.material.baseColorTexture  # type: ignore[union-attr]
                if mesh.visual.material.baseColorTexture is None:  # type: ignore[union-attr]
                    raise ValueError()
            except Exception:
                # Try vertex colors instead.
                try:
                    colors = mesh.visual.to_color().vertex_colors  # type: ignore[union-attr]
                    if len(colors) == 4:
                        # If trimesh gives us a single vertex color for the entire mesh, we can interpret that
                        # as an albedo factor for the whole primitive.
                        albedo_factor = np.array(colors)
                    else:
                        vertex_colors = colors
                except Exception:
                    pass

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

    if children:
        for child in children:
            log_scene(scene, child, path)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs a 3D scene as raw Mesh3D data or as a prepacked Asset3D.",
    )
    parser.add_argument(
        "--scene",
        type=str,
        choices=AVAILABLE_MESHES,
        default=AVAILABLE_MESHES[0],
        help="The name of the scene to load",
    )
    parser.add_argument(
        "--scene-path",
        type=Path,
        help="Path to a scene to analyze. If set, overrides the `--scene` argument.",
    )
    parser.add_argument(
        "--asset3d",
        action="store_true",
        help="Log the scene as a prepacked Asset3D instead of converting it into Mesh3D archetypes.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    scene_path = args.scene_path
    if scene_path is None:
        scene_path = ensure_mesh_downloaded(args.scene)
    blueprint = rrb.Horizontal(
        rrb.Spatial3DView(name="Mesh", origin="/world"),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    )

    rr.script_setup(args, "rerun_example_raw_mesh", default_blueprint=blueprint)
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    # glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
    if args.asset3d:
        rr.log("world", rr.ViewCoordinates.RUB, static=True)
        rr.log("world/asset", rr.Asset3D(path=scene_path))
    else:
        scene = load_scene(scene_path)
        root = next(iter(scene.graph.nodes))
        rr.log(root, rr.ViewCoordinates.RUB, static=True)
        log_scene(scene, root)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
