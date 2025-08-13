from __future__ import annotations

from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt

from rerun.components.image_format import ImageFormat
from rerun.datatypes.channel_datatype import ChannelDatatype
from rerun.datatypes.color_model import ColorModel

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import datatypes

    ImageLike = Union[
        npt.NDArray[np.float16],
        npt.NDArray[np.float32],
        npt.NDArray[np.float64],
        npt.NDArray[np.int16],
        npt.NDArray[np.int32],
        npt.NDArray[np.int64],
        npt.NDArray[np.int8],
        npt.NDArray[np.uint16],
        npt.NDArray[np.uint32],
        npt.NDArray[np.uint64],
        npt.NDArray[np.uint8],
    ]


def _to_numpy(tensor: ImageLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)
    except AttributeError:
        return np.asarray(tensor)


class Mesh3DExt:
    """Extension for [Mesh3D][rerun.archetypes.Mesh3D]."""

    def __init__(
        self: Any,
        *,
        vertex_positions: datatypes.Vec3DArrayLike,
        triangle_indices: datatypes.UVec3DArrayLike | None = None,
        vertex_normals: datatypes.Vec3DArrayLike | None = None,
        vertex_colors: datatypes.Rgba32ArrayLike | None = None,
        vertex_texcoords: datatypes.Vec2DArrayLike | None = None,
        albedo_texture: ImageLike | None = None,
        albedo_factor: datatypes.Rgba32Like | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
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
        class_ids:
            Optional class Ids for the vertices.
            The class ID provides colors and labels if not specified explicitly.

        """

        albedo_texture_buffer = None
        albedo_texture_format = None

        if albedo_texture is not None:
            albedo_texture = _to_numpy(albedo_texture)

            if len(albedo_texture.shape) != 3:
                _send_warning_or_raise("Bad albedo texture shape: {albedo_texture.shape}")
            else:
                h = albedo_texture.shape[0]
                w = albedo_texture.shape[1]
                c = albedo_texture.shape[2]
                if c not in (3, 4):
                    _send_warning_or_raise("Bad albedo texture shape: {albedo_texture.shape}")
                else:
                    color_model = ColorModel.RGB if c == 3 else ColorModel.RGBA
                    try:
                        datatype = ChannelDatatype.from_np_dtype(albedo_texture.dtype)
                        albedo_texture_buffer = albedo_texture.tobytes()
                        albedo_texture_format = ImageFormat(
                            width=w,
                            height=h,
                            color_model=color_model,
                            channel_datatype=datatype,
                        )
                    except KeyError:
                        _send_warning_or_raise(f"Unsupported dtype {albedo_texture.dtype} for Mesh3D:s albedo texture")

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
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
