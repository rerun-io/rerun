# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/mesh3d.fbs".

# You can extend this class by creating a "Mesh3DExt" class in "mesh3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from .mesh3d_ext import Mesh3DExt

__all__ = ["Mesh3D"]


@define(str=False, repr=False, init=False)
class Mesh3D(Mesh3DExt, Archetype):
    """
    A 3D triangle mesh as specified by its per-mesh and per-vertex properties.

    Examples
    --------
    Simple indexed 3D mesh:
    ```python
    import rerun as rr
    from rerun.components import Material

    rr.init("rerun_example_mesh3d_indexed", spawn=True)

    rr.log(
        "triangle",
        rr.Mesh3D(
            vertex_positions=[[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            vertex_normals=[0.0, 0.0, 1.0],
            vertex_colors=[[0, 0, 255], [0, 255, 0], [255, 0, 0]],
            indices=[2, 1, 0],
            mesh_material=Material(albedo_factor=[0xCC, 0x00, 0xCC, 0xFF]),
        ),
    )
    ```

    3D mesh with partial updates:
    ```python
    import numpy as np
    import rerun as rr
    from rerun.components import Position3DBatch

    rr.init("rerun_example_mesh3d_partial_updates", spawn=True)

    vertex_positions = np.array([[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], dtype=np.float32)

    # Log the initial state of our triangle
    rr.set_time_sequence("frame", 0)
    rr.log(
        "triangle",
        rr.Mesh3D(
            vertex_positions=vertex_positions,
            vertex_normals=[0.0, 0.0, 1.0],
            vertex_colors=[[255, 0, 0], [0, 255, 0], [0, 0, 255]],
        ),
    )

    # Only update its vertices' positions each frame
    factors = np.abs(np.sin(np.arange(1, 300, dtype=np.float32) * 0.04))
    for i, factor in enumerate(factors):
        rr.set_time_sequence("frame", i)
        rr.log_components("triangle", [Position3DBatch(vertex_positions * factor)])
    ```
    """

    # __init__ can be found in mesh3d_ext.py

    vertex_positions: components.Position3DBatch = field(
        metadata={"component": "required"},
        converter=components.Position3DBatch._required,  # type: ignore[misc]
    )
    """
    The positions of each vertex.

    If no `indices` are specified, then each triplet of positions is interpreted as a triangle.
    """

    mesh_properties: components.MeshPropertiesBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MeshPropertiesBatch._optional,  # type: ignore[misc]
    )
    """
    Optional properties for the mesh as a whole (including indexed drawing).
    """

    vertex_normals: components.Vector3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Vector3DBatch._optional,  # type: ignore[misc]
    )
    """
    An optional normal for each vertex.

    If specified, this must have as many elements as `vertex_positions`.
    """

    vertex_colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    """
    An optional color for each vertex.
    """

    mesh_material: components.MaterialBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.MaterialBatch._optional,  # type: ignore[misc]
    )
    """
    Optional material properties for the mesh as a whole.
    """

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    """
    Optional class Ids for the vertices.

    The class ID provides colors and labels if not specified explicitly.
    """

    instance_keys: components.InstanceKeyBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyBatch._optional,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual vertex in the mesh.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__


if hasattr(Mesh3DExt, "deferred_patch_class"):
    Mesh3DExt.deferred_patch_class(Mesh3D)
