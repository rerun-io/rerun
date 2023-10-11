from __future__ import annotations

from enum import Enum
from pathlib import Path

import numpy as np
import numpy.typing as npt
from typing_extensions import deprecated  # type: ignore[misc, unused-ignore]

from rerun._log import log
from rerun.archetypes import Asset3D
from rerun.components import MediaType, OutOfTreeTransform3DBatch
from rerun.datatypes import TranslationAndMat3x3
from rerun.log_deprecated.log_decorator import log_decorator
from rerun.recording_stream import RecordingStream

from .._image import ImageEncoded, ImageFormat

__all__ = [
    "MeshFormat",
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


@deprecated(
    """Please migrate to `rr.log(…, rr.Asset3D(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
)
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.Asset3D][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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

    if mesh_path is not None:
        asset3d = Asset3D(path=mesh_path)
    elif mesh_bytes is not None:
        if mesh_format == MeshFormat.GLB:
            media_type = MediaType.GLB
        else:
            media_type = MediaType.OBJ
        asset3d = Asset3D(contents=mesh_bytes, media_type=media_type)
    else:
        raise ValueError("must specify either `mesh_path` or `mesh_bytes`")

    if transform is not None:
        transform = np.require(transform, dtype="float32")
        translation = transform[..., -1]
        mat = [transform[..., 0], transform[..., 1], transform[..., 2]]
        asset3d.transform = OutOfTreeTransform3DBatch(TranslationAndMat3x3(translation=translation, mat3x3=mat))

    return log(entity_path, asset3d, timeless=timeless, recording=recording)


@deprecated(
    """Please migrate to `rr.log(…, rr.ImageEncoded(…))`.
  See: https://www.rerun.io/docs/reference/migration-0-9 for more details."""
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

    !!! Warning "Deprecated"
        Please migrate to [rerun.log][] with [rerun.ImageEncoded][].

        See [the migration guide](https://www.rerun.io/docs/reference/migration-0-9) for more details.

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

    log(
        entity_path,
        ImageEncoded(
            path=img_path,
            contents=img_bytes,
            format=img_format,
        ),
        timeless=timeless,
        recording=recording,
    )
