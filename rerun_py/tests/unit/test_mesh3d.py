from __future__ import annotations

import itertools
from typing import Any, Optional, cast

import rerun as rr
from rerun.components import MaterialBatch, Position3DBatch, UVector3DBatch, Vector3DBatch
from rerun.components.texcoord2d import Texcoord2DBatch
from rerun.datatypes import (
    ClassIdArrayLike,
    Material,
    MaterialLike,
    Rgba32ArrayLike,
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

mesh_materials: list[MaterialLike | None] = [
    None,
    Material(albedo_factor=0xAA0000CC),
]


def mesh_material_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, Material(albedo_factor=0xAA0000CC))

    return MaterialBatch._optional(expected)


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
        mesh_materials,
        class_ids_arrays,
    )

    for (
        vertex_positions,
        vertex_normals,
        vertex_colors,
        vertex_texcoords,
        triangle_indices,
        mesh_material,
        class_ids,
    ) in all_arrays:
        vertex_positions = vertex_positions if vertex_positions is not None else vertex_positions_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        vertex_positions = cast(Vec3DArrayLike, vertex_positions)
        vertex_normals = cast(Optional[Vec3DArrayLike], vertex_normals)
        vertex_colors = cast(Optional[Rgba32ArrayLike], vertex_colors)
        vertex_texcoords = cast(Optional[Vec2DArrayLike], vertex_texcoords)
        triangle_indices = cast(Optional[UVec3DArrayLike], triangle_indices)
        mesh_material = cast(Optional[MaterialLike], mesh_material)
        class_ids = cast(Optional[ClassIdArrayLike], class_ids)

        print(
            f"E: rr.Mesh3D(\n"
            f"    vertex_positions={vertex_positions}\n"
            f"    vertex_normals={vertex_normals}\n"
            f"    vertex_colors={vertex_colors}\n"
            f"    vertex_texcoords={vertex_texcoords}\n"
            f"    triangle_indices={triangle_indices}\n"
            f"    mesh_material={mesh_material}\n"
            f"    class_ids={class_ids}\n"
            f")"
        )
        arch = rr.Mesh3D(
            vertex_positions=vertex_positions,
            vertex_normals=vertex_normals,
            vertex_colors=vertex_colors,
            vertex_texcoords=vertex_texcoords,
            triangle_indices=triangle_indices,
            mesh_material=mesh_material,
            class_ids=class_ids,
        )
        print(f"A: {arch}\n")

        assert arch.vertex_positions == vec3ds_expected(vertex_positions, Position3DBatch)
        assert arch.vertex_normals == vec3ds_expected(vertex_normals, Vector3DBatch)
        assert arch.vertex_colors == colors_expected(vertex_colors)
        assert arch.vertex_texcoords == vec2ds_expected(vertex_texcoords, Texcoord2DBatch)
        assert arch.triangle_indices == uvec3ds_expected(triangle_indices, UVector3DBatch)
        assert arch.mesh_material == mesh_material_expected(mesh_material)
        assert arch.class_ids == class_ids_expected(class_ids)


def test_nullable_albedo_factor() -> None:
    # NOTE: We're just making sure that this doesn't crash… trust me, it used to.
    assert (
        len(
            MaterialBatch([
                Material(albedo_factor=[0xCC, 0x00, 0xCC, 0xFF]),
            ])
        )
        == 1
    )
    assert (
        len(
            MaterialBatch([
                Material(),
            ])
        )
        == 1
    )


if __name__ == "__main__":
    test_mesh3d()
