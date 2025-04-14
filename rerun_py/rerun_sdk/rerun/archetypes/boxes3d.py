# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/boxes3d.fbs".

# You can extend this class by creating a "Boxes3DExt" class in "boxes3d_ext.py".

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
from .boxes3d_ext import Boxes3DExt

__all__ = ["Boxes3D"]


@define(str=False, repr=False, init=False)
class Boxes3D(Boxes3DExt, Archetype):
    """
    **Archetype**: 3D boxes with half-extents and optional center, rotations, colors etc.

    Note that orienting and placing the box is handled via `[archetypes.InstancePoses3D]`.
    Some of its component are repeated here for convenience.
    If there's more instance poses than half sizes, the last half size will be repeated for the remaining poses.

    Example
    -------
    ### Batch of 3D boxes:
    ```python
    import rerun as rr

    rr.init("rerun_example_box3d_batch", spawn=True)

    rr.log(
        "batch",
        rr.Boxes3D(
            centers=[[2, 0, 0], [-2, 0, 0], [0, 0, 2]],
            half_sizes=[[2.0, 2.0, 1.0], [1.0, 1.0, 0.5], [2.0, 0.5, 1.0]],
            quaternions=[
                rr.Quaternion.identity(),
                rr.Quaternion(xyzw=[0.0, 0.0, 0.382683, 0.923880]),  # 45 degrees around Z
            ],
            radii=0.025,
            colors=[(255, 0, 0), (0, 255, 0), (0, 0, 255)],
            fill_mode="solid",
            labels=["red", "green", "blue"],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/1200w.png">
      <img src="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in boxes3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            half_sizes=None,
            centers=None,
            rotation_axis_angles=None,
            quaternions=None,
            colors=None,
            radii=None,
            fill_mode=None,
            labels=None,
            show_labels=None,
            class_ids=None,
        )

    @classmethod
    def _clear(cls) -> Boxes3D:
        """Produce an empty Boxes3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> Boxes3D:
        """
        Update only some specific fields of a `Boxes3D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        half_sizes:
            All half-extents that make up the batch of boxes.
        centers:
            Optional center positions of the boxes.

            If not specified, the centers will be at (0, 0, 0).
            Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        fill_mode:
            Optionally choose whether the boxes are drawn with lines or solid.
        labels:
            Optional text labels for the boxes.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "half_sizes": half_sizes,
                "centers": centers,
                "rotation_axis_angles": rotation_axis_angles,
                "quaternions": quaternions,
                "colors": colors,
                "radii": radii,
                "fill_mode": fill_mode,
                "labels": labels,
                "show_labels": show_labels,
                "class_ids": class_ids,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Boxes3D:
        """Clear all the fields of a `Boxes3D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        half_sizes: datatypes.Vec3DArrayLike | None = None,
        centers: datatypes.Vec3DArrayLike | None = None,
        rotation_axis_angles: datatypes.RotationAxisAngleArrayLike | None = None,
        quaternions: datatypes.QuaternionArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        fill_mode: components.FillModeArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        half_sizes:
            All half-extents that make up the batch of boxes.
        centers:
            Optional center positions of the boxes.

            If not specified, the centers will be at (0, 0, 0).
            Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        rotation_axis_angles:
            Rotations via axis + angle.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        quaternions:
            Rotations via quaternion.

            If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
            Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
        colors:
            Optional colors for the boxes.
        radii:
            Optional radii for the lines that make up the boxes.
        fill_mode:
            Optionally choose whether the boxes are drawn with lines or solid.
        labels:
            Optional text labels for the boxes.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                half_sizes=half_sizes,
                centers=centers,
                rotation_axis_angles=rotation_axis_angles,
                quaternions=quaternions,
                colors=colors,
                radii=radii,
                fill_mode=fill_mode,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "half_sizes": half_sizes,
            "centers": centers,
            "rotation_axis_angles": rotation_axis_angles,
            "quaternions": quaternions,
            "colors": colors,
            "radii": radii,
            "fill_mode": fill_mode,
            "labels": labels,
            "show_labels": show_labels,
            "class_ids": class_ids,
        }
        columns = []

        for batch in batches:
            arrow_array = batch.as_arrow_array()

            # For primitive arrays and fixed size list arrays, we infer partition size from the input shape.
            if pa.types.is_primitive(arrow_array.type) or pa.types.is_fixed_size_list(arrow_array.type):
                param = kwargs[batch.component_descriptor().archetype_field_name]  # type: ignore[index]
                shape = np.shape(param)  # type: ignore[arg-type]
                elem_flat_len = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                if pa.types.is_fixed_size_list(arrow_array.type) and arrow_array.type.list_size == elem_flat_len:
                    # If the product of the last dimensions of the shape are equal to the size of the fixed size list array,
                    # we have `num_rows` single element batches (each element is a fixed sized list).
                    # (This should have been already validated by conversion to the arrow_array)
                    batch_length = 1
                else:
                    batch_length = int(np.prod(shape[1:])) if len(shape) > 1 else 1  # type: ignore[redundant-expr,misc]

                num_rows = shape[0] if len(shape) >= 1 else 1  # type: ignore[redundant-expr,misc]
                sizes = batch_length * np.ones(num_rows)
            else:
                # For non-primitive types, default to partitioning each element separately.
                sizes = np.ones(len(arrow_array))

            columns.append(batch.partition(sizes))

        indicator_column = cls.indicator().partition(np.zeros(len(sizes)))
        return ComponentColumnList([indicator_column] + columns)

    half_sizes: components.HalfSize3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.HalfSize3DBatch._converter,  # type: ignore[misc]
    )
    # All half-extents that make up the batch of boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    centers: components.PoseTranslation3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseTranslation3DBatch._converter,  # type: ignore[misc]
    )
    # Optional center positions of the boxes.
    #
    # If not specified, the centers will be at (0, 0, 0).
    # Note that this uses a [`components.PoseTranslation3D`][rerun.components.PoseTranslation3D] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotation_axis_angles: components.PoseRotationAxisAngleBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationAxisAngleBatch._converter,  # type: ignore[misc]
    )
    # Rotations via axis + angle.
    #
    # If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationAxisAngle`][rerun.components.PoseRotationAxisAngle] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    quaternions: components.PoseRotationQuatBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.PoseRotationQuatBatch._converter,  # type: ignore[misc]
    )
    # Rotations via quaternion.
    #
    # If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    # Note that this uses a [`components.PoseRotationQuat`][rerun.components.PoseRotationQuat] which is also used by [`archetypes.InstancePoses3D`][rerun.archetypes.InstancePoses3D].
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the lines that make up the boxes.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    fill_mode: components.FillModeBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.FillModeBatch._converter,  # type: ignore[misc]
    )
    # Optionally choose whether the boxes are drawn with lines or solid.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # Optional text labels for the boxes.
    #
    # If there's a single label present, it will be placed at the center of the entity.
    # Otherwise, each instance will have its own label.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    show_labels: components.ShowLabelsBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ShowLabelsBatch._converter,  # type: ignore[misc]
    )
    # Optional choice of whether the text labels should be shown by default.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ClassIdBatch._converter,  # type: ignore[misc]
    )
    # Optional [`components.ClassId`][rerun.components.ClassId]s for the boxes.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
