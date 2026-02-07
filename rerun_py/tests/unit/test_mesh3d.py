from __future__ import annotations

import itertools
from typing import Any, cast

import rerun as rr
from rerun.components import AlbedoFactorBatch, Position3DBatch, TriangleIndicesBatch, Vector3DBatch
from rerun.components.texcoord2d import Texcoord2DBatch
from rerun.datatypes import (
    ClassIdArrayLike,
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


if __name__ == "__main__":
    test_mesh3d()
