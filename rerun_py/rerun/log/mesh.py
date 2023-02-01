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
    timeless: bool = False,
) -> None:
    positions = positions.flatten().astype(np.float32)
    if indices is not None:
        indices = indices.flatten().astype(np.uint32)
    if normals is not None:
        normals = normals.flatten().astype(np.float32)

    # Mesh arrow handling happens inside the python bridge
    bindings.log_meshes(entity_path, [positions.flatten()], [indices], [normals], timeless)


def log_meshes(
    entity_path: str,
    positions: Sequence[npt.NDArray[np.float32]],
    *,
    indices: Sequence[Optional[npt.NDArray[np.uint32]]],
    normals: Sequence[Optional[npt.NDArray[np.float32]]],
    timeless: bool = False,
) -> None:
    positions = [p.flatten().astype(np.float32) for p in positions]
    if indices is not None:
        indices = [i.flatten().astype(np.uint32) if i else None for i in indices]
    if normals is not None:
        normals = [n.flatten().astype(np.float32) if n else None for n in normals]

    # Mesh arrow handling happens inside the python bridge
    bindings.log_mesh(entity_path, positions, indices, normals, timeless)
