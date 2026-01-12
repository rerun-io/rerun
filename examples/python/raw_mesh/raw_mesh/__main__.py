#!/usr/bin/env python3
"""
Shows how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy.

Note that while this example loads GLTF meshes to illustrate
[`Mesh3D`](https://rerun.io/docs/reference/types/archetypes/mesh3d)'s abilitites,
you can also send various kinds of mesh assets directly via
[`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d).
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import cast

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
import trimesh

from .download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded

DESCRIPTION = """
# Raw meshes
This example shows how you can log a hierarchical 3D mesh, including its transform hierarchy.

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
        description="Logs raw 3D meshes and their transform hierarchy using the Rerun SDK.",
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
    rr.script_add_args(parser)
    args = parser.parse_args()

    scene_path = args.scene_path
    if scene_path is None:
        scene_path = ensure_mesh_downloaded(args.scene)
    scene = load_scene(scene_path)

    root = next(iter(scene.graph.nodes))

    blueprint = rrb.Horizontal(
        rrb.Spatial3DView(name="Mesh", origin="/world"),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    )

    rr.script_setup(args, "rerun_example_raw_mesh", default_blueprint=blueprint)
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    # glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
    rr.log(root, rr.ViewCoordinates.RUB, static=True)
    log_scene(scene, root)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
