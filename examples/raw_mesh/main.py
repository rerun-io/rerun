#!/usr/bin/env python3

"""
This example demonstrates how to use the Rerun SDK to log raw 3D meshes (so-called
"triangle soups") and their transform hierarchy.

Run:
```sh
# assuming your virtual env is up
examples/raw_mesh/main.py
```
"""

import argparse
from pathlib import Path
from typing import Optional, cast

import numpy as np
import trimesh
from dataset.dataset import AVAILABLE_MESHES, ensure_mesh_downloaded

import rerun as rr


def load_scene(path: Path) -> trimesh.Scene:
    print(f"loading scene {path}…")
    mesh = trimesh.load(path)
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
        # TODO(cmc): Not ideal that the user has to decompose the matrix before logging it.
        if parent:
            world_from_mesh = node_data[0]
            t = trimesh.transformations.translation_from_matrix(world_from_mesh)
            q = trimesh.transformations.quaternion_from_matrix(world_from_mesh)
            # `trimesh` stores quaternions in `wxyz` format, rerun needs `xyzw`
            q = np.array([q[1], q[2], q[3], q[0]])
            rr.log_rigid3(path, parent_from_child=(t, q))

        # Log this node's mesh, if it has one.
        mesh = cast(trimesh.Trimesh, scene.geometry.get(node_data[1]))
        if mesh:
            rr.log_mesh(path, mesh.vertices, indices=mesh.faces, normals=mesh.vertex_normals)

    if children:
        for child in children:
            log_scene(scene, child, path)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs raw 3D meshes and their transform hierarchy using the Rerun SDK."
    )
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--connect", dest="connect", action="store_true", help="Connect to an external viewer")
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
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
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    args = parser.parse_args()

    rr.init("raw_mesh")

    if args.serve:
        rr.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rr.connect(args.addr)
    elif args.save is None and not args.headless:
        rr.spawn_and_connect()

    scene_path = args.scene_path
    if scene_path is None:
        scene_path = ensure_mesh_downloaded(args.scene)
    scene = load_scene(scene_path)

    root = next(iter(scene.graph.nodes))

    # glTF always uses a right-handed coordinate system when +Y is up and meshes face +Z.
    rr.log_view_coordinates(root, xyz="RUB", timeless=True)
    log_scene(scene, root)

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep

            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rr.save(args.save)


if __name__ == "__main__":
    main()
