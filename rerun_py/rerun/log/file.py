from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Optional

import numpy as np
import numpy.typing as npt

from rerun import bindings

__all__ = [
    "MeshFormat",
    "ImageFormat",
    "log_mesh_file",
    "log_image_file",
]


class MeshFormat(Enum):
    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    # GLTF = "GLTF"

    # Binary glTF
    GLB = "GLB"

    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    # Wavefront .obj
    OBJ = "OBJ"


@dataclass
class ImageFormat(Enum):
    # """ jpeg """"
    JPEG = "jpeg"


def log_mesh_file(
    entity_path: str,
    mesh_format: MeshFormat,
    mesh_file: bytes,
    *,
    transform: Optional[npt.NDArray[np.float32]] = None,
    timeless: bool = False,
) -> None:
    """
    Log the contents of a mesh file (.gltf, .glb, .obj, …).

    `transform` is an optional 3x4 affine transform matrix applied to the mesh.

    Example:
    -------
    ```
    # Move mesh 10 units along the X axis.
    transform=np.array([
        [1, 0, 0, 10],
        [0, 1, 0, 0],
        [0, 0, 1, 0]])
    ```

    """
    if transform is None:
        transform = np.empty(shape=(0, 0), dtype=np.float32)
    else:
        transform = np.require(transform, dtype="float32")

    # Mesh arrow handling happens inside the python bridge
    bindings.log_mesh_file(entity_path, mesh_format.value, mesh_file, transform, timeless)


def log_image_file(
    entity_path: str,
    img_path: Path,
    img_format: Optional[ImageFormat] = None,
    timeless: bool = False,
) -> None:
    """
    Log the contents of an image file (only JPEGs supported for now).

    If no `img_format` is specified, we will try and guess it.
    """
    img_format = getattr(img_format, "value", None)

    # Image file arrow handling happens inside the python bridge
    bindings.log_image_file(entity_path, img_path, img_format, timeless)
