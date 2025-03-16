# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/transform3d.fbs".

# You can extend this class by creating a "Transform3DExt" class in "transform3d_ext.py".

from __future__ import annotations

import numpy as np
import pyarrow as pa
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumnList,
)
from ..error_utils import catch_and_log_exceptions
from .transform3d_ext import Transform3DExt

__all__ = ["Transform3D"]


@define(str=False, repr=False, init=False)
class Transform3D(Transform3DExt, Archetype):
    """
    **Archetype**: A transform between two 3D spaces, i.e. a pose.

    From the point of view of the entity's coordinate system,
    all components are applied in the inverse order they are listed here.
    E.g. if both a translation and a max3x3 transform are present,
    the 3x3 matrix is applied first, followed by the translation.

    Whenever you log this archetype, it will write all components, even if you do not explicitly set them.
    This means that if you first log a transform with only a translation, and then log one with only a rotation,
    it will be resolved to a transform with only a rotation.

    For transforms that affect only a single entity and do not propagate along the entity tree refer to [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].

    Examples
    --------
    ### Variety of 3D transforms:
    ```python
    from math import pi

    import rerun as rr
    from rerun.datatypes import Angle, RotationAxisAngle

    rr.init("rerun_example_transform3d", spawn=True)

    arrow = rr.Arrows3D(origins=[0, 0, 0], vectors=[0, 1, 0])

    rr.log("base", arrow)

    rr.log("base/translated", rr.Transform3D(translation=[1, 0, 0]))
    rr.log("base/translated", arrow)

    rr.log(
        "base/rotated_scaled",
        rr.Transform3D(
            rotation=RotationAxisAngle(axis=[0, 0, 1], angle=Angle(rad=pi / 4)),
            scale=2,
        ),
    )
    rr.log("base/rotated_scaled", arrow)
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/1200w.png">
      <img src="https://static.rerun.io/transform3d_simple/141368b07360ce3fcb1553079258ae3f42bdb9ac/full.png" width="640">
    </picture>
    </center>

    ### Transform hierarchy:
    ```python
    import numpy as np
    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_transform3d_hierarchy", spawn=True)

    # One space with the sun in the center, and another one with the planet.
    rr.send_blueprint(
        rrb.Horizontal(rrb.Spatial3DView(origin="sun"), rrb.Spatial3DView(origin="sun/planet", contents="sun/**")),
    )

    rr.set_time("sim_time", timedelta=0)

    # Planetary motion is typically in the XY plane.
    rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

    # Setup points, all are in the center of their own space:
    # TODO(#1361): Should use spheres instead of points.
    rr.log("sun", rr.Points3D([0.0, 0.0, 0.0], radii=1.0, colors=[255, 200, 10]))
    rr.log("sun/planet", rr.Points3D([0.0, 0.0, 0.0], radii=0.4, colors=[40, 80, 200]))
    rr.log("sun/planet/moon", rr.Points3D([0.0, 0.0, 0.0], radii=0.15, colors=[180, 180, 180]))

    # Draw fixed paths where the planet & moon move.
    d_planet = 6.0
    d_moon = 3.0
    angles = np.arange(0.0, 1.01, 0.01) * np.pi * 2
    circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0], dtype=np.float32).transpose()
    rr.log("sun/planet_path", rr.LineStrips3D(circle * d_planet))
    rr.log("sun/planet/moon_path", rr.LineStrips3D(circle * d_moon))

    # Movement via transforms.
    for i in range(6 * 120):
        time = i / 120.0
        rr.set_time("sim_time", timedelta=time)
        r_moon = time * 5.0
        r_planet = time * 2.0

        rr.log(
            "sun/planet",
            rr.Transform3D(
                translation=[np.sin(r_planet) * d_planet, np.cos(r_planet) * d_planet, 0.0],
                rotation=rr.RotationAxisAngle(axis=(1, 0, 0), degrees=20),
            ),
        )
        rr.log(
            "sun/planet/moon",
            rr.Transform3D(
                translation=[np.cos(r_moon) * d_moon, np.sin(r_moon) * d_moon, 0.0],
                from_parent=True,
            ),
        )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/1200w.png">
      <img src="https://static.rerun.io/transform_hierarchy/cb7be7a5a31fcb2efc02ba38e434849248f87554/full.png" width="640">
    </picture>
    </center>

    ### Update a transform over time:
    ```python
    import math

    import rerun as rr


    def truncated_radians(deg: float) -> float:
        return float(int(math.radians(deg) * 1000.0)) / 1000.0


    rr.init("rerun_example_transform3d_row_updates", spawn=True)

    rr.set_time("tick", sequence=0)
    rr.log(
        "box",
        rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
        rr.Transform3D(clear=False, axis_length=10),
    )

    for t in range(100):
        rr.set_time("tick", sequence=t + 1)
        rr.log(
            "box",
            rr.Transform3D(
                clear=False,
                translation=[0, 0, t / 10.0],
                rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=truncated_radians(t * 4)),
            ),
        )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1200w.png">
      <img src="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/full.png" width="640">
    </picture>
    </center>

    ### Update a transform over time, in a single operation:
    ```python
    import math

    import rerun as rr


    def truncated_radians(deg: float) -> float:
        return float(int(math.radians(deg) * 1000.0)) / 1000.0


    rr.init("rerun_example_transform3d_column_updates", spawn=True)

    rr.set_time("tick", sequence=0)
    rr.log(
        "box",
        rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
        rr.Transform3D(clear=False, axis_length=10),
    )

    rr.send_columns(
        "box",
        times=[rr.TimeColumn("tick", sequence=range(1, 101))],
        columns=rr.Transform3D.columns(
            translation=[[0, 0, t / 10.0] for t in range(100)],
            rotation_axis_angle=[
                rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=truncated_radians(t * 4)) for t in range(100)
            ],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/1200w.png">
      <img src="https://static.rerun.io/transform3d_column_updates/80634e1c7c7a505387e975f25ea8b6bc1d4eb9db/full.png" width="640">
    </picture>
    </center>

    ### Update specific properties of a transform over time:
    ```python
    import math

    import rerun as rr


    def truncated_radians(deg: float) -> float:
        return float(int(math.radians(deg) * 1000.0)) / 1000.0


    rr.init("rerun_example_transform3d_partial_updates", spawn=True)

    # Set up a 3D box.
    rr.log(
        "box",
        rr.Boxes3D(half_sizes=[4.0, 2.0, 1.0], fill_mode=rr.components.FillMode.Solid),
        rr.Transform3D(clear=False, axis_length=10),
    )

    # Update only the rotation of the box.
    for deg in range(46):
        rad = truncated_radians(deg * 4)
        rr.log(
            "box",
            rr.Transform3D.from_fields(
                rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
            ),
        )

    # Update only the position of the box.
    for t in range(51):
        rr.log(
            "box",
            rr.Transform3D.from_fields(translation=[0, 0, t / 10.0]),
        )

    # Update only the rotation of the box.
    for deg in range(46):
        rad = truncated_radians((deg + 45) * 4)
        rr.log(
            "box",
            rr.Transform3D.from_fields(
                rotation_axis_angle=rr.RotationAxisAngle(axis=[0.0, 1.0, 0.0], radians=rad),
            ),
        )

    # Clear all of the box's attributes, and reset its axis length.
    rr.log(
        "box",
        rr.Transform3D.from_fields(clear_unset=True, axis_length=15),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/1200w.png">
      <img src="https://static.rerun.io/transform3d_partial_updates/11815bebc69ae400847896372b496cdd3e9b19fb/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in transform3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            translation=None,
            rotation_axis_angle=None,
            quaternion=None,
            scale=None,
            mat3x3=None,
            relation=None,
            axis_length=None,
        )

    @classmethod
    def _clear(cls) -> Transform3D:
        """Produce an empty Transform3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        translation: datatypes.Vec3DLike | None = None,
        rotation_axis_angle: datatypes.RotationAxisAngleLike | None = None,
        quaternion: datatypes.QuaternionLike | None = None,
        scale: datatypes.Vec3DLike | None = None,
        mat3x3: datatypes.Mat3x3Like | None = None,
        relation: components.TransformRelationLike | None = None,
        axis_length: datatypes.Float32Like | None = None,
    ) -> Transform3D:
        """
        Update only some specific fields of a `Transform3D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        translation:
            Translation vector.
        rotation_axis_angle:
            Rotation via axis + angle.
        quaternion:
            Rotation via quaternion.
        scale:
            Scaling factor.
        mat3x3:
            3x3 transformation matrix.
        relation:
            Specifies the relation this transform establishes between this entity and its parent.
        axis_length:
            Visual length of the 3 axes.

            The length is interpreted in the local coordinate system of the transform.
            If the transform is scaled, the axes will be scaled accordingly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "translation": translation,
                "rotation_axis_angle": rotation_axis_angle,
                "quaternion": quaternion,
                "scale": scale,
                "mat3x3": mat3x3,
                "relation": relation,
                "axis_length": axis_length,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Transform3D:
        """Clear all the fields of a `Transform3D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        translation: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angle: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternion: datatypes.QuaternionArrayLike | None = None,
        scale: datatypes.Vec3DArrayLike | None = None,
        mat3x3: datatypes.Mat3x3ArrayLike | None = None,
        relation: components.TransformRelationArrayLike | None = None,
        axis_length: datatypes.Float32ArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        translation:
            Translation vector.
        rotation_axis_angle:
            Rotation via axis + angle.
        quaternion:
            Rotation via quaternion.
        scale:
            Scaling factor.
        mat3x3:
            3x3 transformation matrix.
        relation:
            Specifies the relation this transform establishes between this entity and its parent.
        axis_length:
            Visual length of the 3 axes.

            The length is interpreted in the local coordinate system of the transform.
            If the transform is scaled, the axes will be scaled accordingly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                translation=translation,
                rotation_axis_angle=rotation_axis_angle,
                quaternion=quaternion,
                scale=scale,
                mat3x3=mat3x3,
                relation=relation,
                axis_length=axis_length,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "translation": translation,
            "rotation_axis_angle": rotation_axis_angle,
            "quaternion": quaternion,
            "scale": scale,
            "mat3x3": mat3x3,
            "relation": relation,
            "axis_length": axis_length,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]

                if pa.types.is_fixed_size_list(arrow_array.type) and len(shape) <= 2:
                    # If shape length is 2 or less, we have `num_rows` single element batches (each element is a fixed sized list).
                    # `shape[1]` should be the length of the fixed sized list.
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = shape[1] if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    translation: components.Translation3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Translation3DBatch._converter,  # type: ignore[misc]
    )
    # Translation vector.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotation_axis_angle: components.RotationAxisAngleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RotationAxisAngleBatch._converter,  # type: ignore[misc]
    )
    # Rotation via axis + angle.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    quaternion: components.RotationQuatBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RotationQuatBatch._converter,  # type: ignore[misc]
    )
    # Rotation via quaternion.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    scale: components.Scale3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Scale3DBatch._converter,  # type: ignore[misc]
    )
    # Scaling factor.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    mat3x3: components.TransformMat3x3Batch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TransformMat3x3Batch._converter,  # type: ignore[misc]
    )
    # 3x3 transformation matrix.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    relation: components.TransformRelationBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TransformRelationBatch._converter,  # type: ignore[misc]
    )
    # Specifies the relation this transform establishes between this entity and its parent.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    axis_length: components.AxisLengthBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.AxisLengthBatch._converter,  # type: ignore[misc]
    )
    # Visual length of the 3 axes.
    #
    # The length is interpreted in the local coordinate system of the transform.
    # If the transform is scaled, the axes will be scaled accordingly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
