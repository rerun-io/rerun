from __future__ import annotations

import itertools
from typing import Any, cast

import numpy as np
import numpy.typing as npt
import pytest
import rerun as rr
from rerun.components import (
    AlbedoFactorBatch,
    ImageBufferBatch,
    ImageFormat,
    ImageFormatBatch,
    Position3DBatch,
    TriangleIndicesBatch,
    Vector3DBatch,
)
from rerun.components.texcoord2d import Texcoord2DBatch
from rerun.encodings import (
    ChannelDatatype,
    ClassIdArrayLike,
    ColorModel,
    Rgba32,
    Rgba32ArrayLike,
    Rgba32Like,
    UVec3DArrayLike,
    Vec2DArrayLike,
    Vec3DArrayLike,
)

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    none_empty_or_value,
    uvec3ds_arrays,
    uvec3ds_expected,
    vec2ds_arrays,
    vec2ds_expected,
    vec3ds_arrays,
    vec3ds_expected,
)

albedo_factors: list[Rgba32Like | None] = [
    None,
    Rgba32(0xAA0000CC),
]


def albedo_factor_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, Rgba32(0xAA0000CC))

    return AlbedoFactorBatch._converter(expected)


def test_mesh3d() -> None:
    vertex_positions_arrays = vec3ds_arrays
    vertex_normals_arrays = vec3ds_arrays
    vertex_colors_arrays = colors_arrays
    vertex_texcoord_arrays = vec2ds_arrays
    triangle_indices_arrays = uvec3ds_arrays

    all_arrays = itertools.zip_longest(
        vertex_positions_arrays,
        vertex_normals_arrays,
        vertex_colors_arrays,
        vertex_texcoord_arrays,
        triangle_indices_arrays,
        albedo_factors,
        class_ids_arrays,
    )

    for (
        vertex_positions,
        vertex_normals,
        vertex_colors,
        vertex_texcoords,
        triangle_indices,
        albedo_factor,
        class_ids,
    ) in all_arrays:
        vertex_positions = vertex_positions if vertex_positions is not None else vertex_positions_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        vertex_positions = cast("Vec3DArrayLike", vertex_positions)
        vertex_normals = cast("Vec3DArrayLike | None", vertex_normals)
        vertex_colors = cast("Rgba32ArrayLike | None", vertex_colors)
        vertex_texcoords = cast("Vec2DArrayLike | None", vertex_texcoords)
        triangle_indices = cast("UVec3DArrayLike | None", triangle_indices)
        albedo_factor = cast("Rgba32Like | None", albedo_factor)
        class_ids = cast("ClassIdArrayLike | None", class_ids)

        print(
            f"E: rr.Mesh3D(\n"
            f"    vertex_positions={vertex_positions}\n"
            f"    vertex_normals={vertex_normals}\n"
            f"    vertex_colors={vertex_colors}\n"
            f"    vertex_texcoords={vertex_texcoords}\n"
            f"    triangle_indices={triangle_indices}\n"
            f"    albedo_factor={albedo_factor}\n"
            f"    class_ids={class_ids}\n"
            f")",
        )
        arch = rr.Mesh3D(
            vertex_positions=vertex_positions,
            vertex_normals=vertex_normals,
            vertex_colors=vertex_colors,
            vertex_texcoords=vertex_texcoords,
            triangle_indices=triangle_indices,
            albedo_factor=albedo_factor,
            class_ids=class_ids,
        )
        print(f"A: {arch}\n")

        assert arch.vertex_positions == vec3ds_expected(vertex_positions, Position3DBatch)
        assert arch.vertex_normals == vec3ds_expected(vertex_normals, Vector3DBatch)
        assert arch.vertex_colors == colors_expected(vertex_colors)
        assert arch.vertex_texcoords == vec2ds_expected(vertex_texcoords, Texcoord2DBatch)
        assert arch.triangle_indices == uvec3ds_expected(triangle_indices, TriangleIndicesBatch)
        assert arch.albedo_factor == albedo_factor_expected(albedo_factor)
        assert arch.class_ids == class_ids_expected(class_ids)


def test_mesh3d_from_albedo_texture() -> None:
    textures: list[npt.NDArray[Any]] = [
        np.arange(2 * 3 * 3, dtype=np.uint8).reshape(2, 3, 3),
        np.arange(4 * 2 * 4, dtype=np.uint8).reshape(4, 2, 4),
        np.linspace(0.0, 1.0, 2 * 2 * 3, dtype=np.float32).reshape(2, 2, 3),
    ]

    for texture in textures:
        update = rr.Mesh3D.from_albedo_texture(texture)
        full = rr.Mesh3D(vertex_positions=[[0.0, 0.0, 0.0]], albedo_texture=texture)

        height, width, channels = texture.shape
        assert update.albedo_texture_buffer == ImageBufferBatch._converter(texture.tobytes())
        assert update.albedo_texture_format == ImageFormatBatch._converter(
            ImageFormat(
                width=width,
                height=height,
                color_model=ColorModel.RGB if channels == 3 else ColorModel.RGBA,
                channel_datatype=ChannelDatatype.from_np_dtype(texture.dtype),
            )
        )
        assert update.albedo_texture_buffer == full.albedo_texture_buffer
        assert update.albedo_texture_format == full.albedo_texture_format

        # Only the texture is updated.
        assert update.vertex_positions is None
        assert update.triangle_indices is None
        assert update.vertex_texcoords is None
        assert update.albedo_factor is None


def test_mesh3d_from_albedo_texture_bad_shape() -> None:
    previous = rr.strict_mode()
    rr.set_strict_mode(True)
    try:
        with pytest.raises(ValueError, match="expected 3 dimensions"):
            rr.Mesh3D.from_albedo_texture(np.zeros((2, 2), dtype=np.uint8))
        with pytest.raises(ValueError, match="expected 3 or 4 channels"):
            rr.Mesh3D.from_albedo_texture(np.zeros((2, 2, 2), dtype=np.uint8))
    finally:
        rr.set_strict_mode(previous)


if __name__ == "__main__":
    test_mesh3d()
    test_mesh3d_from_albedo_texture()
    test_mesh3d_from_albedo_texture_bad_shape()
