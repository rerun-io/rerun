# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/points3d.fbs".

# You can extend this class by creating a "Points3DExt" class in "points3d_ext.py".

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
from .points3d_ext import Points3DExt

__all__ = ["Points3D"]


@define(str=False, repr=False, init=False)
class Points3D(Points3DExt, Archetype):
    """
    **Archetype**: A 3D point cloud with positions and optional colors, radii, labels, etc.

    Examples
    --------
    ### Simple 3D points:
    ```python
    import rerun as rr

    rr.init("rerun_example_points3d", spawn=True)

    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1200w.png">
      <img src="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/full.png" width="640">
    </picture>
    </center>

    ### Update a point cloud over time:
    ```python
    import numpy as np
    import rerun as rr

    rr.init("rerun_example_points3d_row_updates", spawn=True)

    # Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    times = np.arange(10, 15, 1.0)
    # fmt: off
    positions = [
        [[1.0, 0.0, 1.0], [0.5, 0.5, 2.0]],
        [[1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0]],
        [[2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5]],
        [[-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5]],
        [[1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0]],
    ]
    # fmt: on

    # At each timestep, all points in the cloud share the same but changing color and radius.
    colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
    radii = [0.05, 0.01, 0.2, 0.1, 0.3]

    for i in range(5):
        rr.set_index("time", timedelta=10 + i)
        rr.log("points", rr.Points3D(positions[i], colors=colors[i], radii=radii[i]))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1200w.png">
      <img src="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/full.png" width="640">
    </picture>
    </center>

    ### Update a point cloud over time, in a single operation:
    ```python
    from __future__ import annotations

    import numpy as np
    import rerun as rr

    rr.init("rerun_example_points3d_column_updates", spawn=True)

    # Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    times = np.arange(10, 15, 1.0)
    # fmt: off
    positions = [
        [1.0, 0.0, 1.0], [0.5, 0.5, 2.0],
        [1.5, -0.5, 1.5], [1.0, 1.0, 2.5], [-0.5, 1.5, 1.0], [-1.5, 0.0, 2.0],
        [2.0, 0.0, 2.0], [1.5, -1.5, 3.0], [0.0, -2.0, 2.5], [1.0, -1.0, 3.5],
        [-2.0, 0.0, 2.0], [-1.5, 1.5, 3.0], [-1.0, 1.0, 3.5],
        [1.0, -1.0, 1.0], [2.0, -2.0, 2.0], [3.0, -1.0, 3.0], [2.0, 0.0, 4.0],
    ]
    # fmt: on

    # At each timestep, all points in the cloud share the same but changing color and radius.
    colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF]
    radii = [0.05, 0.01, 0.2, 0.1, 0.3]

    rr.send_columns(
        "points",
        indexes=[rr.IndexColumn("time", timedelta=times)],
        columns=[
            *rr.Points3D.columns(positions=positions).partition(lengths=[2, 4, 4, 3, 4]),
            *rr.Points3D.columns(colors=colors, radii=radii),
        ],
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1200w.png">
      <img src="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/full.png" width="640">
    </picture>
    </center>

    ### Update specific properties of a point cloud over time:
    ```python
    import rerun as rr

    rr.init("rerun_example_points3d_partial_updates", spawn=True)

    positions = [[i, 0, 0] for i in range(10)]

    rr.set_index("frame", sequence=0)
    rr.log("points", rr.Points3D(positions))

    for i in range(10):
        colors = [[20, 200, 20] if n < i else [200, 20, 20] for n in range(10)]
        radii = [0.6 if n < i else 0.2 for n in range(10)]

        # Update only the colors and radii, leaving everything else as-is.
        rr.set_index("frame", sequence=i)
        rr.log("points", rr.Points3D.from_fields(radii=radii, colors=colors))

    # Update the positions and radii, and clear everything else in the process.
    rr.set_index("frame", sequence=20)
    rr.log("points", rr.Points3D.from_fields(clear_unset=True, positions=positions, radii=0.3))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1200w.png">
      <img src="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in points3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            positions=None,
            radii=None,
            colors=None,
            labels=None,
            show_labels=None,
            class_ids=None,
            keypoint_ids=None,
        )

    @classmethod
    def _clear(cls) -> Points3D:
        """Produce an empty Points3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        positions: datatypes.Vec3DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
    ) -> Points3D:
        """
        Update only some specific fields of a `Points3D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        positions:
            All the 3D positions at which the point cloud shows points.
        radii:
            Optional radii for the points, effectively turning them into circles.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.
        labels:
            Optional text labels for the points.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional class Ids for the points.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
        keypoint_ids:
            Optional keypoint IDs for the points, identifying them within a class.

            If keypoint IDs are passed in but no [`components.ClassId`][rerun.components.ClassId]s were specified, the [`components.ClassId`][rerun.components.ClassId] will
            default to 0.
            This is useful to identify points within a single classification (which is identified
            with `class_id`).
            E.g. the classification might be 'Person' and the keypoints refer to joints on a
            detected skeleton.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            kwargs = {
                "positions": positions,
                "radii": radii,
                "colors": colors,
                "labels": labels,
                "show_labels": show_labels,
                "class_ids": class_ids,
                "keypoint_ids": keypoint_ids,
            }

            if clear_unset:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def cleared(cls) -> Points3D:
        """Clear all the fields of a `Points3D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        positions: datatypes.Vec3DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
    ) -> ComponentColumnList:
        """
        Construct a new column-oriented component bundle.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        The returned columns will be partitioned into unit-length sub-batches by default.
        Use `ComponentColumnList.partition` to repartition the data as needed.

        Parameters
        ----------
        positions:
            All the 3D positions at which the point cloud shows points.
        radii:
            Optional radii for the points, effectively turning them into circles.
        colors:
            Optional colors for the points.

            The colors are interpreted as RGB or RGBA in sRGB gamma-space,
            As either 0-1 floats or 0-255 integers, with separate alpha.
        labels:
            Optional text labels for the points.

            If there's a single label present, it will be placed at the center of the entity.
            Otherwise, each instance will have its own label.
        show_labels:
            Optional choice of whether the text labels should be shown by default.
        class_ids:
            Optional class Ids for the points.

            The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
        keypoint_ids:
            Optional keypoint IDs for the points, identifying them within a class.

            If keypoint IDs are passed in but no [`components.ClassId`][rerun.components.ClassId]s were specified, the [`components.ClassId`][rerun.components.ClassId] will
            default to 0.
            This is useful to identify points within a single classification (which is identified
            with `class_id`).
            E.g. the classification might be 'Person' and the keypoints refer to joints on a
            detected skeleton.

        """

        inst = cls.__new__(cls)
        with catch_and_log_exceptions(context=cls.__name__):
            inst.__attrs_init__(
                positions=positions,
                radii=radii,
                colors=colors,
                labels=labels,
                show_labels=show_labels,
                class_ids=class_ids,
                keypoint_ids=keypoint_ids,
            )

        batches = inst.as_component_batches(include_indicators=False)
        if len(batches) == 0:
            return ComponentColumnList([])

        kwargs = {
            "positions": positions,
            "radii": radii,
            "colors": colors,
            "labels": labels,
            "show_labels": show_labels,
            "class_ids": class_ids,
            "keypoint_ids": keypoint_ids,
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

    positions: components.Position3DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Position3DBatch._converter,  # type: ignore[misc]
    )
    # All the 3D positions at which the point cloud shows points.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.RadiusBatch._converter,  # type: ignore[misc]
    )
    # Optional radii for the points, effectively turning them into circles.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.ColorBatch._converter,  # type: ignore[misc]
    )
    # Optional colors for the points.
    #
    # The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    # As either 0-1 floats or 0-255 integers, with separate alpha.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.TextBatch._converter,  # type: ignore[misc]
    )
    # Optional text labels for the points.
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
    # Optional class Ids for the points.
    #
    # The [`components.ClassId`][rerun.components.ClassId] provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    keypoint_ids: components.KeypointIdBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.KeypointIdBatch._converter,  # type: ignore[misc]
    )
    # Optional keypoint IDs for the points, identifying them within a class.
    #
    # If keypoint IDs are passed in but no [`components.ClassId`][rerun.components.ClassId]s were specified, the [`components.ClassId`][rerun.components.ClassId] will
    # default to 0.
    # This is useful to identify points within a single classification (which is identified
    # with `class_id`).
    # E.g. the classification might be 'Person' and the keypoints refer to joints on a
    # detected skeleton.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
