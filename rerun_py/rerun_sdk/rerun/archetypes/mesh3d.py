# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/mesh3d.fbs".

# You can extend this class by creating a "Mesh3DExt" class in "mesh3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
)
from .mesh3d_ext import Mesh3DExt

__all__ = ["Mesh3D"]


@define(str=False, repr=False, init=False)
class Mesh3D(Mesh3DExt, Archetype):
    """
    **Archetype**: A 3D triangle mesh as specified by its per-mesh and per-vertex properties.

    See also [`archetypes.Asset3D`][rerun.archetypes.Asset3D].

    If there are multiple [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D] instances logged to the same entity as a mesh,
    an instance of the mesh will be drawn for each transform.

    Examples
    --------
    ### Simple indexed 3D mesh:
    ```python
    import rerun as rr

    rr.init("rerun_example_mesh3d_indexed", spawn=True)

    rr.log(
        "triangle",
        rr.Mesh3D(
            vertex_positions=[[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            vertex_normals=[0.0, 0.0, 1.0],
            vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
            triangle_indices=[2, 1, 0],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/1200w.png">
      <img src="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/full.png" width="640">
    </picture>
    </center>

    ### 3D mesh with instancing:
    ```python
    import rerun as rr

    rr.init("rerun_example_mesh3d_instancing", spawn=True)
    rr.set_time_sequence("frame", 0)

    rr.log(
        "shape",
        rr.Mesh3D(
            vertex_positions=[[1, 1, 1], [-1, -1, 1], [-1, 1, -1], [1, -1, -1]],
            triangle_indices=[[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]],
            vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]],
        ),
    )
    # This box will not be affected by its parent's instance poses!
    rr.log(
        "shape/box",
        rr.Boxes3D(half_sizes=[[5.0, 5.0, 5.0]]),
    )

    for i in range(0, 100):
        rr.set_time_sequence("frame", i)
        rr.log(
            "shape",
            rr.InstancePoses3D(
                translations=[[2, 0, 0], [0, 2, 0], [0, -2, 0], [-2, 0, 0]],
                rotation_axis_angles=rr.RotationAxisAngle([0, 0, 1], rr.Angle(deg=i * 2)),
            ),
        )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1200w.png">
      <img src="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in mesh3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            vertex_positions=None,  # type: ignore[arg-type]
            triangle_indices=None,  # type: ignore[arg-type]
            vertex_normals=None,  # type: ignore[arg-type]
            vertex_colors=None,  # type: ignore[arg-type]
            vertex_texcoords=None,  # type: ignore[arg-type]
            albedo_factor=None,  # type: ignore[arg-type]
            albedo_texture_buffer=None,  # type: ignore[arg-type]
            albedo_texture_format=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Mesh3D:
        """Produce an empty Mesh3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def update_fields(
        cls,
        *,
        clear: bool = False,
        vertex_positions: datatypes.Vec3DArrayLike | None = None,
        triangle_indices: datatypes.UVec3DArrayLike | None = None,
        vertex_normals: datatypes.Vec3DArrayLike | None = None,
        vertex_colors: datatypes.Rgba32ArrayLike | None = None,
        vertex_texcoords: datatypes.Vec2DArrayLike | None = None,
        albedo_factor: datatypes.Rgba32Like | None = None,
        albedo_texture_buffer: datatypes.BlobLike | None = None,
        albedo_texture_format: datatypes.ImageFormatLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Mesh3D:
        """
        Update only some specific fields of a `Mesh3D`.

        Parameters
        ----------
        clear:
            If true, all unspecified fields will be explicitly cleared.
        vertex_positions:
            The positions of each vertex.

            If no `triangle_indices` are specified, then each triplet of positions is interpreted as a triangle.
        triangle_indices:
            Optional indices for the triangles that make up the mesh.
        vertex_normals:
            An optional normal for each vertex.
        vertex_colors:
            An optional color for each vertex.
        vertex_texcoords:
            An optional uv texture coordinate for each vertex.
        albedo_factor:
            A color multiplier applied to the whole mesh.
        albedo_texture_buffer:
            Optional albedo texture.

            Used with the [`components.Texcoord2D`][rerun.components.Texcoord2D] of the mesh.

            Currently supports only sRGB(A) textures, ignoring alpha.
            (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
        albedo_texture_format:
            The format of the `albedo_texture_buffer`, if any.
        class_ids:
            Optional class Ids for the vertices.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        kwargs = {
            "vertex_positions": vertex_positions,
            "triangle_indices": triangle_indices,
            "vertex_normals": vertex_normals,
            "vertex_colors": vertex_colors,
            "vertex_texcoords": vertex_texcoords,
            "albedo_factor": albedo_factor,
            "albedo_texture_buffer": albedo_texture_buffer,
            "albedo_texture_format": albedo_texture_format,
            "class_ids": class_ids,
        }

        if clear:
            kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

        return Mesh3D(**kwargs)  # type: ignore[arg-type]

    @classmethod
    def clear_fields(cls) -> Mesh3D:
        """Clear all the fields of a `Mesh3D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            vertex_positions=[],  # type: ignore[arg-type]
            triangle_indices=[],  # type: ignore[arg-type]
            vertex_normals=[],  # type: ignore[arg-type]
            vertex_colors=[],  # type: ignore[arg-type]
            vertex_texcoords=[],  # type: ignore[arg-type]
            albedo_factor=[],  # type: ignore[arg-type]
            albedo_texture_buffer=[],  # type: ignore[arg-type]
            albedo_texture_format=[],  # type: ignore[arg-type]
            class_ids=[],  # type: ignore[arg-type]
        )
        return inst

    vertex_positions: components.Position3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Position3DBatch._optional,  # type: ignore[misc]
    )
    # The positions of each vertex.
    #
    # If no `triangle_indices` are specified, then each triplet of positions is interpreted as a triangle.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    triangle_indices: components.TriangleIndicesBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TriangleIndicesBatch._optional,  # type: ignore[misc]
    )
    # Optional indices for the triangles that make up the mesh.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    vertex_normals: components.Vector3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Vector3DBatch._optional,  # type: ignore[misc]
    )
    # An optional normal for each vertex.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    vertex_colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # An optional color for each vertex.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    vertex_texcoords: components.Texcoord2DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Texcoord2DBatch._optional,  # type: ignore[misc]
    )
    # An optional uv texture coordinate for each vertex.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    albedo_factor: components.AlbedoFactorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.AlbedoFactorBatch._optional,  # type: ignore[misc]
    )
    # A color multiplier applied to the whole mesh.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    albedo_texture_buffer: components.ImageBufferBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ImageBufferBatch._optional,  # type: ignore[misc]
    )
    # Optional albedo texture.
    #
    # Used with the [`components.Texcoord2D`][rerun.components.Texcoord2D] of the mesh.
    #
    # Currently supports only sRGB(A) textures, ignoring alpha.
    # (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    albedo_texture_format: components.ImageFormatBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ImageFormatBatch._optional,  # type: ignore[misc]
    )
    # The format of the `albedo_texture_buffer`, if any.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional class Ids for the vertices.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
