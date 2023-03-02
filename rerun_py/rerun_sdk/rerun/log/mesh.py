from typing import Optional, Sequence

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.log.log_decorator import log_decorator

__all__ = [
    "log_mesh",
    "log_meshes",
]


@log_decorator
def log_mesh(
    entity_path: str,
    positions: npt.ArrayLike,
    *,
    indices: Optional[npt.ArrayLike] = None,
    normals: Optional[npt.ArrayLike] = None,
    albedo_factor: Optional[npt.ArrayLike] = None,
    timeless: bool = False,
) -> None:
    """
    Log a raw 3D mesh by specifying its vertex positions, and optionally indices, normals and albedo factor.

    The data is _always_ interpreted as a triangle list:

    * `positions` is a (potentially flattened) array of 3D points, i.e. the total number of elements must be divisible
      by 3.
    * `indices`, if specified, is a flattened array of indices that describe the mesh's faces,
      i.e. its length must be divisible by 3.
    * `normals`, if specified, is a (potentially flattened) array of 3D vectors that describe the normal for each
      vertex, i.e. the total number of elements must be divisible by 3 and more importantly, `len(normals)` should be
      equal to `len(positions)`.
    * `albedo_factor`, if specified, is either a linear, unmultiplied, normalized RGB (vec3) or RGBA (vec4) value.

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
        An array of 3D points
    indices:
        Optional array of indices that describe the mesh's faces
    normals:
        Optional array of 3D vectors that describe the normal of each vertices
    albedo_factor:
        Optional RGB(A) color for the albedo factor of the mesh, aka base color factor.
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

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(entity_path, [positions.flatten()], [indices], [normals], [albedo_factor], timeless)


@log_decorator
def log_meshes(
    entity_path: str,
    position_buffers: Sequence[npt.ArrayLike],
    *,
    index_buffers: Sequence[Optional[npt.ArrayLike]],
    normal_buffers: Sequence[Optional[npt.ArrayLike]],
    albedo_factors: Sequence[Optional[npt.ArrayLike]],
    timeless: bool = False,
) -> None:
    """
    Log multiple raw 3D meshes by specifying their different buffers and albedo factors.

    To learn more about how the data within these buffers is interpreted and laid out, refer
    to `log_mesh`'s documentation.

    * If specified, `index_buffers` must have the same length as `position_buffers`.
    * If specified, `normal_buffers` must have the same length as `position_buffers`.
    * If specified, `albedo_factors` must have the same length as `position_buffers`.

    Parameters
    ----------
    entity_path:
        Path to the mesh in the space hierarchy
    position_buffers:
        A sequence of position buffers, one for each mesh.
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
    if index_buffers is not None:
        index_buffers = [np.asarray(i, dtype=np.uint32).flatten() if i else None for i in index_buffers]
    if normal_buffers is not None:
        normal_buffers = [np.asarray(n, dtype=np.float32).flatten() if n else None for n in normal_buffers]
    if albedo_factors is not None:
        albedo_factors = [np.asarray(af, dtype=np.float32).flatten() if af else None for af in albedo_factors]

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(entity_path, position_buffers, index_buffers, normal_buffers, albedo_factors, timeless)
