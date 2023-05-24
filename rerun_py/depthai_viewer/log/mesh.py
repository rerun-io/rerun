from typing import Any, Optional, Sequence

import numpy as np
import numpy.typing as npt

from depthai_viewer import bindings
from depthai_viewer.log import (
    Colors,
    _normalize_colors,
)
from depthai_viewer.log.log_decorator import log_decorator

__all__ = [
    "log_mesh",
    "log_meshes",
]


@log_decorator
def log_mesh(
    entity_path: str,
    positions: Any,
    *,
    indices: Optional[Any] = None,
    normals: Optional[Any] = None,
    albedo_factor: Optional[Any] = None,
    vertex_colors: Optional[Colors] = None,
    timeless: bool = False,
) -> None:
    """
    Log a raw 3D mesh by specifying its vertex positions, and optionally indices, normals and albedo factor.

    You can also use [`rerun.log_mesh_file`] to log .gltf, .glb, .obj, etc.

    Example:
    -------
    ```
    # A simple red triangle:
    rerun.log_mesh(
        "world/mesh",
        positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0]
        ],
        indices = [0, 1, 2],
        normals = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0]
        ],
        albedo_factor = [1.0, 0.0, 0.0],
    )
    ```

    Parameters
    ----------
    entity_path:
        Path to the mesh in the space hierarchy
    positions:
        An array of 3D points.
        If no `indices` are specified, then each triplet of positions is interpreted as a triangle.
    indices:
        If specified, is a flattened array of indices that describe the mesh's triangles,
        i.e. its length must be divisible by 3.
    normals:
        If specified, is a (potentially flattened) array of 3D vectors that describe the normal for each
        vertex, i.e. the total number of elements must be divisible by 3 and more importantly, `len(normals)` should be
        equal to `len(positions)`.
    albedo_factor:
        Optional color multiplier of the mesh using RGB or unmuliplied RGBA in linear 0-1 space.
    vertex_colors:
        Optional array of RGB(A) vertex colors, in sRGB gamma space, either as 0-1 floats or 0-255 integers.
        If specified, the alpha is considered separate (unmultiplied).
    timeless:
        If true, the mesh will be timeless (default: False)

    """

    positions = np.asarray(positions, dtype=np.float32).flatten()

    if indices is not None:
        indices = np.asarray(indices, dtype=np.uint32).flatten()
    if normals is not None:
        normals = np.asarray(normals, dtype=np.float32).flatten()
    if albedo_factor is not None:
        albedo_factor = np.asarray(albedo_factor, dtype=np.float32).flatten()
    if vertex_colors is not None:
        vertex_colors = _normalize_colors(vertex_colors)

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(
        entity_path,
        position_buffers=[positions.flatten()],
        vertex_color_buffers=[vertex_colors],
        index_buffers=[indices],
        normal_buffers=[normals],
        albedo_factors=[albedo_factor],
        timeless=timeless,
    )


@log_decorator
def log_meshes(
    entity_path: str,
    position_buffers: Sequence[npt.ArrayLike],
    *,
    vertex_color_buffers: Sequence[Optional[Colors]],
    index_buffers: Sequence[Optional[npt.ArrayLike]],
    normal_buffers: Sequence[Optional[npt.ArrayLike]],
    albedo_factors: Sequence[Optional[npt.ArrayLike]],
    timeless: bool = False,
) -> None:
    """
    Log multiple raw 3D meshes by specifying their different buffers and albedo factors.

    To learn more about how the data within these buffers is interpreted and laid out, refer
    to the documentation for [`rerun.log_mesh`].

    Parameters
    ----------
    entity_path:
        Path to the mesh in the space hierarchy
    position_buffers:
        A sequence of position buffers, one for each mesh.
    vertex_color_buffers:
        An optional sequence of vertex color buffers, one for each mesh.
    index_buffers:
        An optional sequence of index buffers, one for each mesh.
    normal_buffers:
        An optional sequence of normal buffers, one for each mesh.
    albedo_factors:
        An optional sequence of albedo factors, one for each mesh.
    timeless:
        If true, the mesh will be timeless (default: False)

    """

    position_buffers = [np.asarray(p, dtype=np.float32).flatten() for p in position_buffers]
    if vertex_color_buffers is not None:
        vertex_color_buffers = [_normalize_colors(c) for c in vertex_color_buffers]
    if index_buffers is not None:
        index_buffers = [np.asarray(i, dtype=np.uint32).flatten() if i else None for i in index_buffers]
    if normal_buffers is not None:
        normal_buffers = [np.asarray(n, dtype=np.float32).flatten() if n else None for n in normal_buffers]
    if albedo_factors is not None:
        albedo_factors = [np.asarray(af, dtype=np.float32).flatten() if af else None for af in albedo_factors]

    # Mesh arrow handling happens inside the python bridge

    bindings.log_meshes(
        entity_path,
        position_buffers=position_buffers,
        vertex_color_buffers=vertex_color_buffers,
        index_buffers=index_buffers,
        normal_buffers=normal_buffers,
        albedo_factors=albedo_factors,
        timeless=timeless,
    )
