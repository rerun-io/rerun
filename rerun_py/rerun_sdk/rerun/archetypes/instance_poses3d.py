# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/instance_poses3d.fbs".

# You can extend this class by creating a "InstancePoses3DExt" class in "instance_poses3d_ext.py".

from __future__ import annotations

from typing import Any

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions

__all__ = ["InstancePoses3D"]


@define(str=False, repr=False, init=False)
class InstancePoses3D(Archetype):
    """
    **Archetype**: One or more transforms between the current entity and its parent. Unlike [`archetypes.Transform3D`][rerun.archetypes.Transform3D], it is *not* propagated in the transform hierarchy.

    If both [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D] and [`archetypes.Transform3D`][rerun.archetypes.Transform3D] are present,
    first the tree propagating [`archetypes.Transform3D`][rerun.archetypes.Transform3D] is applied, then [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].

    From the point of view of the entity's coordinate system,
    all components are applied in the inverse order they are listed here.
    E.g. if both a translation and a max3x3 transform are present,
    the 3x3 matrix is applied first, followed by the translation.

    Currently, many visualizers support only a single instance transform per entity.
    Check archetype documentations for details - if not otherwise specified, only the first instance transform is applied.
    Some visualizers like the mesh visualizer used for [`archetypes.Mesh3D`][rerun.archetypes.Mesh3D],
    will draw an object for every pose, a behavior also known as "instancing".

    Example
    -------
    ### Regular & instance transforms in tandem:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_instance_pose3d_combined", spawn=True)

    rr.set_index("frame", sequence=0)

    # Log a box and points further down in the hierarchy.
    rr.log("world/box", rr.Boxes3D(half_sizes=[[1.0, 1.0, 1.0]]))
    rr.log("world/box/points", rr.Points3D(np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-10, 10, 10j)]]]).T))

    for i in range(180):
        rr.set_index("frame", sequence=i)

        # Log a regular transform which affects both the box and the points.
        rr.log("world/box", rr.Transform3D(rotation_axis_angle=rr.RotationAxisAngle([0, 0, 1], angle=rr.Angle(deg=i * 2))))

        # Log an instance pose which affects only the box.
        rr.log("world/box", rr.InstancePoses3D(translations=[0, 0, abs(i * 0.1 - 5.0) - 5.0]))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/1200w.png">
      <img src="https://static.rerun.io/leaf_transform3d/41674f0082d6de489f8a1cd1583f60f6b5820ddf/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self: Any,
        *,
        translations: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        scales: datatypes.Vec3DArrayLike | None = None,
        mat3x3: datatypes.Mat3x3ArrayLike | None = None,
    ) -> None:
        """
        Create a new instance of the InstancePoses3D archetype.

        Parameters
        ----------
        translations:
            Translation vectors.
        rotation_axis_angles:
            Rotations via axis + angle.
        quaternions:
            Rotations via quaternion.
        scales:
            Scaling factors.
        mat3x3:
            3x3 transformation matrices.

        """

        # You can define your own __init__ function as a member of InstancePoses3DExt in instance_poses3d_ext.py
        with catch_and_log_exceptions(context=self.__class__.__name__):
            self.__attrs_init__(
                translations=translations,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                scales=scales,
                mat3x3=mat3x3,
            )
            return
        self.__attrs_clear__()

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            translations=None,
            rotation_axis_angles=None,
            quaternions=None,
            scales=None,
            mat3x3=None,
        )

    @classmethod
    def _clear(cls) -> InstancePoses3D:
        """Produce an empty InstancePoses3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        translations: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        scales: datatypes.Vec3DArrayLike | None = None,
        mat3x3: datatypes.Mat3x3ArrayLike | None = None,
    ) -> InstancePoses3D:
        """
        Update only some specific fields of a `InstancePoses3D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        translations:
            Translation vectors.
        rotation_axis_angles:
            Rotations via axis + angle.
        quaternions:
            Rotations via quaternion.
        scales:
            Scaling factors.
        mat3x3:
            3x3 transformation matrices.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "translations": translations,
                "rotation_axis_angles": rotation_axis_angles,
                "quaternions": quaternions,
                "scales": scales,
                "mat3x3": mat3x3,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> InstancePoses3D:
        """Clear all the fields of a `InstancePoses3D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        translations: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        scales: datatypes.Vec3DArrayLike | None = None,
        mat3x3: datatypes.Mat3x3ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        translations:
            Translation vectors.
        rotation_axis_angles:
            Rotations via axis + angle.
        quaternions:
            Rotations via quaternion.
        scales:
            Scaling factors.
        mat3x3:
            3x3 transformation matrices.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                translations=translations,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                scales=scales,
                mat3x3=mat3x3,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "translations": translations,
            "rotation_axis_angles": rotation_axis_angles,
            "quaternions": quaternions,
            "scales": scales,
            "mat3x3": mat3x3,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]
                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    translations: components.PoseTranslation3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseTranslation3DBatch._converter,  # type: ignore[misc]
    )
    # Translation vectors.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotation_axis_angles: components.PoseRotationAxisAngleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationAxisAngleBatch._converter,  # type: ignore[misc]
    )
    # Rotations via axis + angle.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    quaternions: components.PoseRotationQuatBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationQuatBatch._converter,  # type: ignore[misc]
    )
    # Rotations via quaternion.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    scales: components.PoseScale3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseScale3DBatch._converter,  # type: ignore[misc]
    )
    # Scaling factors.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    mat3x3: components.PoseTransformMat3x3Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseTransformMat3x3Batch._converter,  # type: ignore[misc]
    )
    # 3x3 transformation matrices.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
