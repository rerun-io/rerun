from typing import Optional, Sequence

import numpy as np
import numpy.typing as npt

from rerun import bindings

__all__ = [
    "log_mesh",
    "log_meshes",
]


def log_mesh(
    entity_path: str,
    positions: npt.NDArray[np.float32],
    *,
    indices: Optional[npt.NDArray[np.uint32]] = None,
    normals: Optional[npt.NDArray[np.float32]] = None,
    albedo_factor: Optional[npt.NDArray[np.float32]] = None,
    timeless: bool = False,
) -> None:
    """
    Log a raw 3D mesh by specifying its vertex positions, and optionally indices, normals and albedo factor.

    The data is _always_ interpreted as a triangle list:

    * `positions` is a flattened array of 3D points, i.e. its length must be divisible by 3.
    * `indices`, if specified, is a flattened array of indices that describe the mesh's faces,
      i.e. its length must be divisible by 3.
    * `normals`, if specified, is a flattened array of 3D vectors that describe the normal
      for each vertex, i.e. its length must be divisible by 3 and more importantly it has to be
      equal to the length of `positions`.
    * `albedo_factor`, if specified, is either a linear, unmultiplied, normalized RGB (vec3) or
      RGBA (vec4) value.

    Example:
    -------
    ```
    # A simple red triangle:
    positions = np.array([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
    indices = np.array([0, 1, 2])
    normals = np.array([0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0])
    albedo_factor = np.array([1.0, 0.0, 0.0])
    ```

    Parameters
    ----------
    entity_path:
        Path to the mesh in the space hierarchy
    positions:
        A flattened array of 3D points
    indices:
        Optional flattened array of indices that describe the mesh's faces
    normals:
        Optional flattened array of 3D vectors that describe the normal of each vertices
    albedo_factor:
        Optional RGB(A) color for the albedo factor of the mesh, aka base color factor.
    timeless:
        If true, the mesh will be timeless (default: False)

    """

    if not bindings.is_enabled():
        return

    positions = positions.flatten().astype(np.float32)
    if indices is not None:
        indices = indices.flatten().astype(np.uint32)
    if normals is not None:
        normals = normals.flatten().astype(np.float32)
    if albedo_factor is not None:
        albedo_factor = albedo_factor.flatten().astype(np.float32)

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(entity_path, [positions.flatten()], [indices], [normals], [albedo_factor], timeless)


def log_meshes(
    entity_path: str,
    position_buffers: Sequence[npt.NDArray[np.float32]],
    *,
    index_buffers: Sequence[Optional[npt.NDArray[np.uint32]]],
    normal_buffers: Sequence[Optional[npt.NDArray[np.float32]]],
    albedo_factors: Sequence[Optional[npt.NDArray[np.float32]]],
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

    if not bindings.is_enabled():
        return

    position_buffers = [p.flatten().astype(np.float32) for p in position_buffers]
    if index_buffers is not None:
        index_buffers = [i.flatten().astype(np.uint32) if i else None for i in index_buffers]
    if normal_buffers is not None:
        normal_buffers = [n.flatten().astype(np.float32) if n else None for n in normal_buffers]
    if albedo_factors is not None:
        albedo_factors = [af.flatten().astype(np.float32) if af else None for af in albedo_factors]

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(entity_path, position_buffers, index_buffers, normal_buffers, albedo_factors, timeless)
