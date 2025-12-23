from __future__ import annotations

import io
import os
from typing import TYPE_CHECKING, cast
from urllib.parse import urlparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
from ament_index_python.packages import get_package_share_directory
from yourdfpy import URDF

if TYPE_CHECKING:
    import trimesh
    from std_msgs.msg import String


def ament_locate_package(fname: str) -> str:
    """Helper to locate urdf resources via ament."""
    if not fname.startswith("package://"):
        return fname
    parsed = urlparse(fname)
    return os.path.join(get_package_share_directory(parsed.netloc), parsed.path[1:])


def load_urdf_from_msg(msg: String) -> URDF:
    """Load a URDF file using `yourdfpy` and find resources via ament."""
    f = io.StringIO(msg.data)
    return URDF.load(f, filename_handler=ament_locate_package)


def log_scene(scene: trimesh.Scene, node: str, path: str | None = None, static: bool = False) -> None:
    """Log a trimesh scene to rerun."""
    path = path + "/" + node if path else node

    parent = scene.graph.transforms.parents.get(node)
    children = scene.graph.transforms.children.get(node)

    node_data = scene.graph.get(frame_to=node, frame_from=parent)

    if node_data:
        # Log the transform between this node and its direct parent (if it has one!).
        if parent:
            world_from_mesh = node_data[0]
            rr.log(
                path,
                rr.Transform3D(
                    translation=world_from_mesh[3, 0:3],
                    mat3x3=world_from_mesh[0:3, 0:3],
                ),
                static=static,
            )

        # Log this node's mesh, if it has one.
        mesh = cast("trimesh.Trimesh", scene.geometry.get(node_data[1]))
        if mesh:
            # If vertex colors are set, use the average color as the albedo factor
            # for the whole mesh.
            mean_vertex_color = None
            try:
                colors = np.mean(mesh.visual.vertex_colors, axis=0)
                if len(colors) == 4:
                    mean_vertex_color = np.array(colors) / 255.0
            except Exception:
                pass

            # If trimesh gives us a single vertex color for the entire mesh, we can interpret that
            # as an albedo factor for the whole primitive.
            visual_color = None
            try:
                colors = mesh.visual.to_color().vertex_colors
                if len(colors) == 4:
                    visual_color = np.array(colors) / 255.0
            except Exception:
                pass

            albedo_factor = mean_vertex_color if mean_vertex_color is not None else visual_color

            rr.log(
                path,
                rr.Mesh3D(
                    vertex_positions=mesh.vertices,
                    triangle_indices=mesh.faces,
                    vertex_normals=mesh.vertex_normals,
                    albedo_factor=albedo_factor,
                ),
                static=static,
            )

    if children:
        for child in children:
            log_scene(scene, child, path, static)
