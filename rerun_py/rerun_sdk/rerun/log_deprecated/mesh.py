from __future__ import annotations

from typing import Any, Sequence

import numpy as np
import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import Mesh3D
from rerun.components import Material, MeshProperties
from rerun.log_deprecated import Colors, _normalize_colors
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "log_mesh",
    "log_meshes",
]


@deprecated(
    """Please migrate to `rr.log(…, rr.Mesh3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_mesh(
    entity_path: str,
    positions: Any,
    *,
    indices: Any | None = None,
    normals: Any | None = None,
    albedo_factor: Any | None = None,
    vertex_colors: Colors | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log a raw 3D mesh by specifying its vertex positions, and optionally indices, normals and albedo factor.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Mesh3D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    positions = np.asarray(positions, dtype=np.float32).flatten()

    if indices is not None:
        indices = np.asarray(indices, dtype=np.uint32).flatten()
    if normals is not None:
        normals = np.asarray(normals, dtype=np.float32).flatten()
    if albedo_factor is not None:
        albedo_factor = np.asarray(albedo_factor, dtype=np.float32).flatten()
    if vertex_colors is not None:
        vertex_colors = _normalize_colors(vertex_colors)

    mesh3d = Mesh3D(
        vertex_positions=positions,
        vertex_normals=normals,
        vertex_colors=vertex_colors,
        mesh_properties=MeshProperties(indices=indices),
        mesh_material=Material(albedo_factor=albedo_factor),
    )
    return log(entity_path, mesh3d, timeless=timeless, recording=recording)


@deprecated(
    """Please migrate to `rr.log(…, rr.Mesh3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
@log_decorator
def log_meshes(
    entity_path: str,
    position_buffers: Sequence[npt.ArrayLike],
    *,
    vertex_color_buffers: Sequence[Colors | None],
    index_buffers: Sequence[npt.ArrayLike | None],
    normal_buffers: Sequence[npt.ArrayLike | None],
    albedo_factors: Sequence[npt.ArrayLike | None],
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log multiple raw 3D meshes by specifying their different buffers and albedo factors.

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Mesh3D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """
    raise ValueError("Logging multiple meshes at the same entity path is not supported")
