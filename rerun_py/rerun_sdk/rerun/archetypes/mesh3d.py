# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/mesh3d.fbs".

# You can extend this class by creating a "Mesh3DExt" class in "mesh3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)

__all__ = ["Mesh3D"]


@define(str=False, repr=False)
class Mesh3D(Archetype):
    """
    A 3D triangle mesh as specified by its per-mesh and per-vertex properties.

    Examples
    --------
    Simple indexed 3D mesh:
    ```python
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_mesh3d_indexed", spawn=True)

    rr2.log(
        "triangle",
        rr2.Mesh3D(
            [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            vertex_normals=[0.0, 0.0, 1.0],
            vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
            mesh_properties=rr2.cmp.MeshProperties(vertex_indices=[2, 1, 0]),
            mesh_material=rr2.cmp.Material(albedo_factor=[0xCC, 0x00, 0xCC, 0xFF]),
        ),
    )
    ```

    3D mesh with partial updates:
    ```python
    import numpy as np
    import rerun as rr
    import rerun.experimental as rr2

    rr.init("rerun_example_mesh3d_partial_updates", spawn=True)

    vertex_positions = np.array([[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32)

    # Log the initial state of our triangle
    rr.set_time_sequence("frame", 0)
    rr2.log(
        "triangle",
        rr2.Mesh3D(
            vertex_positions,
            vertex_normals=[0.0, 0.0, 1.0],
            vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        ),
    )

    # Only update its vertices' positions each frame
    factors = np.abs(np.sin(np.arange(1, 300, dtype=np.float32) * 0.04))
    for i, factor in enumerate(factors):
        rr.set_time_sequence("frame", i)
        rr2.log_components("triangle", [rr2.cmp.Position3DArray.from_similar(vertex_positions * factor)])
    ```
    """

    # You can define your own __init__ function as a member of Mesh3DExt in mesh3d_ext.py

    vertex_positions: components.Position3DArray = field(
        metadata={"component": "required"},
        converter=components.Position3DArray.from_similar,  # type: ignore[misc]
    )
    """
    The positions of each vertex.

    If no `indices` are specified, then each triplet of positions is interpreted as a triangle.
    """

    mesh_properties: components.MeshPropertiesArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MeshPropertiesArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional properties for the mesh as a whole (including indexed drawing).
    """

    vertex_normals: components.Vector3DArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Vector3DArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    An optional normal for each vertex.

    If specified, this must have as many elements as `vertex_positions`.
    """

    vertex_colors: components.ColorArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    An optional color for each vertex.
    """

    mesh_material: components.MaterialArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MaterialArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional material properties for the mesh as a whole.
    """

    class_ids: components.ClassIdArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Optional class Ids for the vertices.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyArray | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyArray.optional_from_similar,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual vertex in the mesh.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__
