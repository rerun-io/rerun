#!/usr/bin/env python3

"""
Shows how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy.

Run:
```sh
# assuming your virtual env is up
examples/python/raw_mesh/main.py
```
"""

import argparse
from pathlib import Path
from typing import Optional, cast

import numpy as np
import rerun as rr  # pip install rerun-sdk
import trimesh
from download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded


def load_scene(path: Path) -> trimesh.Scene:
    print(f"loading scene {path}…")
    mesh = trimesh.load(path, force="scene")
    return cast(trimesh.Scene, mesh)


# NOTE: The scene hierarchy will look different compared to the Rust example, as this is using the
# trimesh hierarchy, not the raw glTF hierarchy.
def log_scene(scene: trimesh.Scene, node: str, path: Optional[str] = None) -> None:
    path = path + "/" + node if path else node

    parent = scene.graph.transforms.parents.get(node)
    children = scene.graph.transforms.children.get(node)

    node_data = scene.graph.get(frame_to=node, frame_from=parent)
    if node_data:
        # Log the transform between this node and its direct parent (if it has one!).
        if parent:
            # TODO(andreas): We should support 4x4 matrices directly
            world_from_mesh = node_data[0]
            rr.log_transform3d(
                path,
                rr.TranslationAndMat3(
                    trimesh.transformations.translation_from_matrix(world_from_mesh), world_from_mesh[0:3, 0:3]
                ),
            )

        # Log this node's mesh, if it has one.
        mesh = cast(trimesh.Trimesh, scene.geometry.get(node_data[1]))
        if mesh:
            albedo_factor = None
            # If trimesh gives us a single vertex color for the entire mesh, we can interpret that
            # as an albedo factor for the whole primitive.
            try:
                colors = mesh.visual.to_color().vertex_colors
                if len(colors) == 4:
                    albedo_factor = np.array(colors) / 255.0
            except Exception:
                pass
            rr.log_mesh(
                path, mesh.vertices, indices=mesh.faces, normals=mesh.vertex_normals, albedo_factor=albedo_factor
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
        "--scene_path",
        type=Path,
        help="Path to a scene to analyze. If set, overrides the `--scene` argument.",
    )
    rr.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    rr.script_setup(args, "raw_mesh")

    scene_path = args.scene_path
    if scene_path is None:
        scene_path = ensure_mesh_downloaded(args.scene)
    scene = load_scene(scene_path)

    root = next(iter(scene.graph.nodes))

    # glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
    rr.log_view_coordinates(root, xyz="RUB", timeless=True)
    log_scene(scene, root)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
