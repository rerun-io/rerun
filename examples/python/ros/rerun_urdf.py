import io
import os
from typing import Optional, cast
from urllib.parse import urlparse

import numpy as np
import depthai_viewer as viewer
import trimesh
from ament_index_python.packages import get_package_share_directory
from std_msgs.msg import String
from yourdfpy import URDF


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


def log_scene(scene: trimesh.Scene, node: str, path: Optional[str] = None, timeless: bool = False) -> None:
    """Log a trimesh scene to rerun."""
    path = path + "/" + node if path else node

    parent = scene.graph.transforms.parents.get(node)
    children = scene.graph.transforms.children.get(node)

    node_data = scene.graph.get(frame_to=node, frame_from=parent)

    if node_data:
        # Log the transform between this node and its direct parent (if it has one!).
        if parent:
            world_from_mesh = node_data[0]
            t = trimesh.transformations.translation_from_matrix(world_from_mesh)
            q = trimesh.transformations.quaternion_from_matrix(world_from_mesh)
            # `trimesh` stores quaternions in `wxyz` format, rerun needs `xyzw`
            # TODO(jleibs): Remove conversion once [#883](https://github.com/rerun-io/rerun/issues/883) is closed
            q = np.array([q[1], q[2], q[3], q[0]])
            viewer.log_rigid3(path, parent_from_child=(t, q), timeless=timeless)

        # Log this node's mesh, if it has one.
        mesh = cast(trimesh.Trimesh, scene.geometry.get(node_data[1]))
        if mesh:
            # If vertex colors are set, use the average color as the albedo factor
            # for the whole mesh.
            vertex_colors = None
            try:
                colors = np.mean(mesh.visual.vertex_colors, axis=0)
                if len(colors) == 4:
                    vertex_colors = np.array(colors) / 255.0
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

            albedo_factor = vertex_colors if vertex_colors is not None else visual_color

            viewer.log_mesh(
                path,
                mesh.vertices,
                indices=mesh.faces,
                normals=mesh.vertex_normals,
                albedo_factor=albedo_factor,
                timeless=timeless,
            )

    if children:
        for child in children:
            log_scene(scene, child, path, timeless)
