from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path

import numpy as np
import numpy.typing as npt

from rerun import bindings
from rerun.log.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

__all__ = [
    "MeshFormat",
    "ImageFormat",
    "log_mesh_file",
    "log_image_file",
]


class MeshFormat(Enum):
    """Mesh file format."""

    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    # GLTF = "GLTF"
    GLB = "GLB"
    """glTF binary format."""

    # Needs some way of logging materials too, or adding some default material to the
    # viewer.
    OBJ = "OBJ"
    """Wavefront .obj format."""


@dataclass
class ImageFormat(Enum):
    """Image file format."""

    JPEG = "jpeg"
    """JPEG format."""

    PNG = "png"
    """PNG format."""


@log_decorator
def log_mesh_file(
    entity_path: str,
    mesh_format: MeshFormat,
    *,
    mesh_bytes: bytes | None = None,
    mesh_path: Path | None = None,
    transform: npt.ArrayLike | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log the contents of a mesh file (.gltf, .glb, .obj, …).

    You must pass either `mesh_bytes` or `mesh_path`.

    You can also use [`rerun.log_mesh`] to log raw mesh data.

    Example:
    -------
    ```
    # Move mesh 10 units along the X axis.
    transform=np.array([
        [1, 0, 0, 10],
        [0, 1, 0, 0],
        [0, 0, 1, 0]])
    ```

    Parameters
    ----------
    entity_path:
        Path to the mesh in the space hierarchy
    mesh_format:
        Format of the mesh file
    mesh_bytes:
        Content of an mesh file, e.g. a `.glb`.
    mesh_path:
        Path to an mesh file, e.g. a `.glb`.
    transform:
        Optional 3x4 affine transform matrix applied to the mesh
    timeless:
        If true, the mesh will be timeless (default: False)
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    if transform is None:
        transform = np.empty(shape=(0, 0), dtype=np.float32)
    else:
        transform = np.require(transform, dtype="float32")

    # Mesh arrow handling happens inside the python bridge
    bindings.log_mesh_file(
        entity_path,
        mesh_format=mesh_format.value,
        mesh_bytes=mesh_bytes,
        mesh_path=mesh_path,
        transform=transform,
        timeless=timeless,
        recording=recording,
    )


@log_decorator
def log_image_file(
    entity_path: str,
    *,
    img_bytes: bytes | None = None,
    img_path: Path | None = None,
    img_format: ImageFormat | None = None,
    timeless: bool = False,
    recording: RecordingStream | None = None,
) -> None:
    """
    Log an image file given its contents or path on disk.

    You must pass either `img_bytes` or `img_path`.

    Only JPEGs and PNGs are supported right now.

    JPEGs will be stored compressed, saving memory,
    whilst PNGs will currently be decoded before they are logged.
    This may change in the future.

    If no `img_format` is specified, rerun will try to guess it.

    Parameters
    ----------
    entity_path:
        Path to the image in the space hierarchy.
    img_bytes:
        Content of an image file, e.g. a `.jpg`.
    img_path:
        Path to an image file, e.g. a `.jpg`.
    img_format:
        Format of the image file.
    timeless:
        If true, the image will be timeless (default: False).
    recording:
        Specifies the [`rerun.RecordingStream`][] to use.
        If left unspecified, defaults to the current active data recording, if there is one.
        See also: [`rerun.init`][], [`rerun.set_global_data_recording`][].

    """

    recording = RecordingStream.to_native(recording)

    img_format = getattr(img_format, "value", None)

    # Image file arrow handling happens inside the python bridge
    bindings.log_image_file(
        entity_path,
        img_bytes=img_bytes,
        img_path=img_path,
        img_format=img_format,
        timeless=timeless,
        recording=recording,
    )
