from __future__ import annotations

from typing import Any

import numpy.typing as npt

from .. import components, datatypes
from ..error_utils import catch_and_log_exceptions


class Mesh3DExt:
    """Extension for [Mesh3D][rerun.archetypes.Mesh3D]."""

    def __init__(
        self: Any,
        *,
        vertex_positions: datatypes.Vec3DArrayLike,
        indices: npt.ArrayLike | None = None,
        mesh_properties: datatypes.MeshPropertiesLike | None = None,
        vertex_normals: datatypes.Vec3DArrayLike | None = None,
        vertex_colors: datatypes.Rgba32ArrayLike | None = None,
        vertex_texcoords: datatypes.Vec2DArrayLike | None = None,
        albedo_texture: datatypes.TensorDataLike | None = None,
        mesh_material: datatypes.MaterialLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ):
        """
        Create a new instance of the Mesh3D archetype.

        Parameters
        ----------
        vertex_positions:
            The positions of each vertex.
            If no `indices` are specified, then each triplet of positions is interpreted as a triangle.
        indices:
            If specified, a flattened array of indices that describe the mesh's triangles,
            i.e. its length must be divisible by 3.
            Mutually exclusive with `mesh_properties`.
        mesh_properties:
            Optional properties for the mesh as a whole (including indexed drawing).
            Mutually exclusive with `indices`.
        vertex_normals:
            An optional normal for each vertex.
            If specified, this must have as many elements as `vertex_positions`.
        vertex_texcoords:
            An optional texture coordinate for each vertex.
            If specified, this must have as many elements as `vertex_positions`.
        vertex_colors:
            An optional color for each vertex.
        mesh_material:
            Optional material properties for the mesh as a whole.
        albedo_texture:
            Optional albedo texture. Used with `vertex_texcoords` on `Mesh3D`.
            Currently supports only sRGB(A) textures, ignoring alpha.
            (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        class_ids:
            Optional class Ids for the vertices.
            The class ID provides colors and labels if not specified explicitly.
        instance_keys:
            Unique identifiers for each individual vertex in the mesh.
        """
        with catch_and_log_exceptions(context=self.__class__.__name__):
            if indices is not None:
                if mesh_properties is not None:
                    raise ValueError("indices and mesh_properties are mutually exclusive")
                mesh_properties = datatypes.MeshProperties(indices=indices)

            # You can define your own __init__ function as a member of Mesh3DExt in mesh3d_ext.py
            self.__attrs_init__(
                vertex_positions=vertex_positions,
                mesh_properties=mesh_properties,
                vertex_normals=vertex_normals,
                vertex_colors=vertex_colors,
                vertex_texcoords=vertex_texcoords,
                albedo_texture=albedo_texture,
                mesh_material=mesh_material,
                class_ids=class_ids,
                instance_keys=instance_keys,
            )
            return

        self.__attrs_clear__()
