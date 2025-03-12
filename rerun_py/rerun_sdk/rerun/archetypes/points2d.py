# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/points2d.fbs".

# You can extend this class by creating a "Points2DExt" class in "points2d_ext.py".

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
from .points2d_ext import Points2DExt

__all__ = ["Points2D"]


@define(str=False, repr=False, init=False)
class Points2D(Points2DExt, Archetype):
    """
    **Archetype**: A 2D point cloud with positions and optional colors, radii, labels, etc.

    Examples
    --------
    ### Randomly distributed 2D points with varying color and radius:
    ```python
    import rerun as rr
    import rerun.blueprint as rrb
    from numpy.random import default_rng

    rr.init("rerun_example_points2d_random", spawn=True)
    rng = default_rng(12345)

    positions = rng.uniform(-3, 3, size=[10, 2])
    colors = rng.uniform(0, 255, size=[10, 4])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("random", rr.Points2D(positions, colors=colors, radii=radii))

    # Set view bounds:
    rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-4, 4], y_range=[-4, 4])))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1200w.png">
      <img src="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/full.png" width="640">
    </picture>
    </center>

    ### Log points with radii given in UI points:
    ```python
    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_points2d_ui_radius", spawn=True)

    # Two blue points with scene unit radii of 0.1 and 0.3.
    rr.log(
        "scene_units",
        rr.Points2D(
            [[0, 0], [0, 1]],
            # By default, radii are interpreted as world-space units.
            radii=[0.1, 0.3],
            colors=[0, 0, 255],
        ),
    )

    # Two red points with ui point radii of 40 and 60.
    # UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
    # For 100% ui scaling, UI points are equal to pixels.
    rr.log(
        "ui_points",
        rr.Points2D(
            [[1, 0], [1, 1]],
            # rr.Radius.ui_points produces radii that the viewer interprets as given in ui points.
            radii=rr.Radius.ui_points([40.0, 60.0]),
            colors=[255, 0, 0],
        ),
    )

    # Set view bounds:
    rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1, 2], y_range=[-1, 2])))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/1200w.png">
      <img src="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/full.png" width="640">
    </picture>
    </center>

    """

    # __init__ can be found in points2d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            positions=None,
            radii=None,
            colors=None,
            labels=None,
            show_labels=None,
            draw_order=None,
            class_ids=None,
            keypoint_ids=None,
        )

    @classmethod
    def _clear(cls) -> Points2D:
        """Produce an empty Points2D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    @classmethod
    def from_fields(
        cls,
        *,
        clear_unset: bool = False,
        positions: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolLike | None = None,
        draw_order: datatypes.Float32Like | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
    ) -> Points2D:
        """
        Update only some specific fields of a `Points2D`.

        Parameters
        ----------
        clear_unset:
            If true, all unspecified fields will be explicitly cleared.
        positions:
            All the 2D positions at which the point cloud shows points.
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
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.
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
                "draw_order": draw_order,
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
    def cleared(cls) -> Points2D:
        """Clear all the fields of a `Points2D`."""
        return cls.from_fields(clear_unset=True)

    @classmethod
    def columns(
        cls,
        *,
        positions: datatypes.Vec2DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        draw_order: datatypes.Float32ArrayLike | None = None,
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
            All the 2D positions at which the point cloud shows points.
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
        draw_order:
            An optional floating point value that specifies the 2D drawing order.

            Objects with higher values are drawn on top of those with lower values.
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
                draw_order=draw_order,
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
            "draw_order": draw_order,
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

    positions: components.Position2DBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.Position2DBatch._converter,  # type: ignore[misc]
    )
    # All the 2D positions at which the point cloud shows points.
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

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": True},
        default=None,
        converter=components.DrawOrderBatch._converter,  # type: ignore[misc]
    )
    # An optional floating point value that specifies the 2D drawing order.
    #
    # Objects with higher values are drawn on top of those with lower values.
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
