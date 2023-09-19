from __future__ import annotations

import itertools
from typing import Any, Optional, cast

import rerun.experimental as rr2
from rerun.experimental import cmp as rrc
from rerun.experimental import dt as rrd

from .common_arrays import (
    class_ids_arrays,
    class_ids_expected,
    colors_arrays,
    colors_expected,
    instance_keys_arrays,
    instance_keys_expected,
    none_empty_or_value,
    vec3ds_arrays,
    vec3ds_expected,
)

mesh_properties_arrays: list[rrd.MeshPropertiesArrayLike] = [
    [],
    rrd.MeshProperties(triangle_indices=[1, 2, 3, 4, 5, 6]),
    [rrd.MeshProperties(triangle_indices=[1, 2, 3, 4, 5, 6])],
]


def mesh_properties_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, rrc.MeshProperties(triangle_indices=[1, 2, 3, 4, 5, 6]))

    return rrc.MeshPropertiesArray.optional_from_similar(expected)


mesh_material_arrays: list[rrd.MaterialArrayLike] = [
    [],
    rrd.Material(albedo_factor=0xAA0000CC),
    [rrd.Material(albedo_factor=0xAA0000CC)],
]


def mesh_material_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, rrc.Material(albedo_factor=0xAA0000CC))

    return rrc.MaterialArray.optional_from_similar(expected)


def test_mesh3d() -> None:
    vertex_positions_arrays = vec3ds_arrays
    vertex_normals_arrays = vec3ds_arrays
    vertex_colors_arrays = colors_arrays

    all_arrays = itertools.zip_longest(
        vertex_positions_arrays,
        vertex_normals_arrays,
        vertex_colors_arrays,
        mesh_properties_arrays,
        mesh_material_arrays,
        class_ids_arrays,
        instance_keys_arrays,
    )

    for (
        vertex_positions,
        vertex_normals,
        vertex_colors,
        mesh_properties,
        mesh_material,
        class_ids,
        instance_keys,
    ) in all_arrays:
        vertex_positions = vertex_positions if vertex_positions is not None else vertex_positions_arrays[-1]
        vertex_normals = vertex_normals if vertex_normals is not None else vertex_normals_arrays[-1]
        mesh_properties = mesh_properties if mesh_properties is not None else mesh_properties_arrays[-1]
        mesh_material = mesh_material if mesh_material is not None else mesh_material_arrays[-1]

        # make Pyright happy as it's apparently not able to track typing info trough zip_longest
        vertex_positions = cast(rrd.Vec3DArrayLike, vertex_positions)
        vertex_normals = cast(Optional[rrd.Vec3DArrayLike], vertex_normals)
        vertex_colors = cast(Optional[rrd.ColorArrayLike], vertex_colors)
        mesh_properties = cast(Optional[rrd.MeshPropertiesArrayLike], mesh_properties)
        mesh_material = cast(Optional[rrd.MaterialArrayLike], mesh_material)
        class_ids = cast(Optional[rrd.ClassIdArrayLike], class_ids)
        instance_keys = cast(Optional[rrc.InstanceKeyArrayLike], instance_keys)

        print(
            f"E: rr2.Mesh3D(\n"
            f"    {vertex_positions}\n"
            f"    vertex_normals={vertex_normals}\n"
            f"    vertex_colors={vertex_colors}\n"
            f"    mesh_properties={mesh_properties}\n"
            f"    mesh_material={mesh_material}\n"
            f"    class_ids={class_ids}\n"
            f"    instance_keys={instance_keys}\n"
            f")"
        )
        arch = rr2.Mesh3D(
            vertex_positions,
            vertex_normals=vertex_normals,
            vertex_colors=vertex_colors,
            mesh_properties=mesh_properties,
            mesh_material=mesh_material,
            class_ids=class_ids,
            instance_keys=instance_keys,
        )
        print(f"A: {arch}\n")

        assert arch.vertex_positions == vec3ds_expected(vertex_positions, rrc.Position3DArray)
        assert arch.vertex_normals == vec3ds_expected(vertex_normals, rrc.Vector3DArray)
        assert arch.vertex_colors == colors_expected(vertex_colors)
        assert arch.mesh_properties == mesh_properties_expected(mesh_properties)
        assert arch.mesh_material == mesh_material_expected(mesh_material)
        assert arch.class_ids == class_ids_expected(class_ids)
        assert arch.instance_keys == instance_keys_expected(instance_keys)


if __name__ == "__main__":
    test_mesh3d()
