from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt

from rerun.components.image_format import ImageFormat
from rerun.encodings.channel_datatype import ChannelDatatype
from rerun.encodings.color_model import ColorModel

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import components, encodings
    from .mesh3d import Mesh3D

    ImageLike = (
        npt.NDArray[np.float16]
        | npt.NDArray[np.float32]
        | npt.NDArray[np.float64]
        | npt.NDArray[np.int16]
        | npt.NDArray[np.int32]
        | npt.NDArray[np.int64]
        | npt.NDArray[np.int8]
        | npt.NDArray[np.uint16]
        | npt.NDArray[np.uint32]
        | npt.NDArray[np.uint64]
        | npt.NDArray[np.uint8]
    )


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)
    except AttributeError:
        return np.asarray(tensor)


def _albedo_texture_fields(albedo_texture: ImageLike) -> tuple[bytes | None, ImageFormat | None]:
    """Convert an image into the `albedo_texture_buffer` and `albedo_texture_format` of a `Mesh3D`."""
    albedo_texture = _to_numpy(albedo_texture)

    if len(albedo_texture.shape) != 3:
        _send_warning_or_raise(f"Bad albedo texture shape: {albedo_texture.shape}, expected 3 dimensions")
        return None, None

    h, w, c = albedo_texture.shape
    if c not in (3, 4):
        _send_warning_or_raise(f"Bad albedo texture shape: {albedo_texture.shape}, expected 3 or 4 channels")
        return None, None

    try:
        datatype = ChannelDatatype.from_np_dtype(albedo_texture.dtype)
    except KeyError:
        _send_warning_or_raise(f"Unsupported dtype {albedo_texture.dtype} for Mesh3D:s albedo texture")
        return None, None

    albedo_texture_format = ImageFormat(
        width=w,
        height=h,
        color_model=ColorModel.RGB if c == 3 else ColorModel.RGBA,
        channel_datatype=datatype,
    )
    return albedo_texture.tobytes(), albedo_texture_format


class Mesh3DExt:
    """Extension for [Mesh3D][rerun.archetypes.Mesh3D]."""

    def __init__(
        self: Any,
        *,
        vertex_positions: encodings.Vec3DArrayLike,
        triangle_indices: encodings.UVec3DArrayLike | None = None,
        vertex_normals: encodings.Vec3DArrayLike | None = None,
        vertex_colors: encodings.Rgba32ArrayLike | None = None,
        vertex_texcoords: encodings.Vec2DArrayLike | None = None,
        albedo_texture: ImageLike | None = None,
        albedo_factor: encodings.Rgba32Like | None = None,
        face_rendering: components.MeshFaceRenderingLike | None = None,
        class_ids: encodings.ClassIdArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the Mesh3D archetype.

        Parameters
        ----------
        vertex_positions:
            The positions of each vertex.
            If no `indices` are specified, then each triplet of positions is interpreted as a triangle.
        triangle_indices:
            Optional indices for the triangles that make up the mesh.
        vertex_normals:
            An optional normal for each vertex.
            If specified, this must have as many elements as `vertex_positions`.
        vertex_texcoords:
            An optional texture coordinate for each vertex.
            If specified, this must have as many elements as `vertex_positions`.
        vertex_colors:
            An optional color for each vertex.
        albedo_factor:
            Optional color multiplier for the whole mesh
        albedo_texture:
            Optional albedo texture. Used with `vertex_texcoords` on `Mesh3D`.
            Currently supports only sRGB(A) textures, ignoring alpha.
            (meaning that the texture must have 3 or 4 channels)
        face_rendering:
            Determines which faces of the mesh are rendered.
            The default is `DoubleSided`, meaning both front and back faces are shown.
        class_ids:
            Optional class Ids for the vertices.
            The class ID provides colors and labels if not specified explicitly.

        """

        albedo_texture_buffer = None
        albedo_texture_format = None

        if albedo_texture is not None:
            albedo_texture_buffer, albedo_texture_format = _albedo_texture_fields(albedo_texture)

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                vertex_positions=vertex_positions,
                triangle_indices=triangle_indices,
                vertex_normals=vertex_normals,
                vertex_colors=vertex_colors,
                vertex_texcoords=vertex_texcoords,
                albedo_texture_buffer=albedo_texture_buffer,
                albedo_texture_format=albedo_texture_format,
                albedo_factor=albedo_factor,
                face_rendering=face_rendering,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()

    @classmethod
    def from_albedo_texture(cls, albedo_texture: ImageLike) -> Mesh3D:
        """
        Update only the albedo texture of a `Mesh3D`.

        Helper for [`Mesh3D.from_fields`][rerun.archetypes.Mesh3D.from_fields] that takes an image,
        like the `albedo_texture` argument of the constructor,
        and sets both `albedo_texture_buffer` and `albedo_texture_format`.
        This makes it possible to change the texture over time without logging the geometry again.

        Parameters
        ----------
        albedo_texture:
            The new albedo texture.
            Currently supports only sRGB(A) textures, ignoring alpha.
            (meaning that the texture must have 3 or 4 channels)

        """
        from .. import Mesh3D

        albedo_texture_buffer, albedo_texture_format = _albedo_texture_fields(albedo_texture)
        return Mesh3D.from_fields(
            albedo_texture_buffer=albedo_texture_buffer,
            albedo_texture_format=albedo_texture_format,
        )
