#!/usr/bin/env python3
"""
Shows how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy.

Run:
```sh
# assuming your virtual env is up
examples/python/raw_mesh/main.py
```
"""
from __future__ import annotations

import argparse
from pathlib import Path
from typing import cast

import numpy as np
import rerun as rr  # pip install rerun-sdk
import trimesh
from download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded
from rerun.components import Material


def load_scene(path: Path) -> trimesh.Scene:
    print(f"loading scene {path}…")
    mesh = trimesh.load(path, force="scene")
    return cast(trimesh.Scene, mesh)


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
            # TODO(andreas): We should support 4x4 matrices directly
            world_from_mesh = node_data[0]
            rr.log(
                path,
                rr.Transform3D(
                    translation=trimesh.transformations.translation_from_matrix(world_from_mesh),
                    mat3x3=world_from_mesh[0:3, 0:3],
                ),
            )

        # Log this node's mesh, if it has one.
        mesh = cast(trimesh.Trimesh, scene.geometry.get(node_data[1]))
        if mesh:
            vertex_colors = None
            mesh_material = None
            try:
                colors = mesh.visual.to_color().vertex_colors
                if len(colors) == 4:
                    # If trimesh gives us a single vertex color for the entire mesh, we can interpret that
                    # as an albedo factor for the whole primitive.
                    mesh_material = Material(albedo_factor=np.array(colors))
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
                    indices=mesh.faces,
                    mesh_material=mesh_material,
                ),
            )

    if children:
        for child in children:
            log_scene(scene, child, path)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs raw 3D meshes and their transform hierarchy using the Rerun SDK."
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

    rr.script_setup(args, "rerun_example_raw_mesh")

    scene_path = args.scene_path
    if scene_path is None:
        scene_path = ensure_mesh_downloaded(args.scene)
    scene = load_scene(scene_path)

    root = next(iter(scene.graph.nodes))

    # glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
    rr.log(root, rr.ViewCoordinates.RUB, timeless=True)
    log_scene(scene, root)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
