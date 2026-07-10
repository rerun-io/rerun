from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np
import numpy.typing as npt

from rerun.components.image_format import ImageFormat
from rerun.datatypes.channel_datatype import ChannelDatatype
from rerun.datatypes.color_model import ColorModel

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from .. import components, datatypes

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


def _texture_to_buffer_and_format(
    texture: ImageLike | None, archetype_name: str
) -> tuple[bytes | None, ImageFormat | None]:
    if texture is None:
        return None, None

    texture = texture if isinstance(texture, np.ndarray) else np.asarray(texture)

    if len(texture.shape) != 3:
        _send_warning_or_raise(f"Bad albedo texture shape: {texture.shape}, expected 3 dimensions")
        return None, None

    h, w, c = texture.shape
    if c not in (3, 4):
        _send_warning_or_raise(f"Bad albedo texture shape: {texture.shape}, expected 3 or 4 channels")
        return None, None

    color_model = ColorModel.RGB if c == 3 else ColorModel.RGBA
    try:
        datatype = ChannelDatatype.from_np_dtype(texture.dtype)
    except KeyError:
        _send_warning_or_raise(f"Unsupported dtype {texture.dtype} for {archetype_name}'s albedo texture")
        return None, None

    return texture.tobytes(), ImageFormat(
        width=w,
        height=h,
        color_model=color_model,
        channel_datatype=datatype,
    )


class Triangles3DExt:
    """Extension for [Triangles3D][rerun.archetypes.Triangles3D]."""

    def __init__(
        self: Any,
        *,
        vertex_positions: datatypes.Vec3DArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        vertex_texcoords: datatypes.Vec2DArrayLike | None = None,
        albedo_texture: ImageLike | None = None,
        albedo_factor: datatypes.Rgba32Like | None = None,
        line_radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> None:
        """Create a new instance of the Triangles3D archetype."""

        albedo_texture_buffer, albedo_texture_format = _texture_to_buffer_and_format(albedo_texture, "Triangles3D")

        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                vertex_positions=vertex_positions,
                colors=colors,
                vertex_texcoords=vertex_texcoords,
                line_radii=line_radii,
                fill_mode=fill_mode,
                albedo_factor=albedo_factor,
                albedo_texture_buffer=albedo_texture_buffer,
                albedo_texture_format=albedo_texture_format,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )
            return

        self.__attrs_clear__()
