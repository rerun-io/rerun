# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/points3d.fbs".

# You can extend this class by creating a "Points3DExt" class in "points3d_ext.py".

from __future__ import annotations

import numpy as np
import numpy.typing as npt
from attrs import define, field

from .. import components, datatypes
from .._baseclasses import (
    Archetype,
    ComponentColumn,
    DescribedComponentBatch,
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
    ### Randomly distributed 3D points with varying color and radius:
    ```python
    import rerun as rr
    from numpy.random import default_rng

    rr.init("rerun_example_points3d_random", spawn=True)
    rng = default_rng(12345)

    positions = rng.uniform(-5, 5, size=[10, 3])
    colors = rng.uniform(0, 255, size=[10, 3])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("random", rr.Points3D(positions, colors=colors, radii=radii))
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1200w.png">
      <img src="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/full.png" width="640">
    </picture>
    </center>

    ### Log points with radii given in UI points:
    ```python
    import rerun as rr

    rr.init("rerun_example_points3d_ui_radius", spawn=True)

    # Two blue points with scene unit radii of 0.1 and 0.3.
    rr.log(
        "scene_units",
        rr.Points3D(
            [[0, 1, 0], [1, 1, 1]],
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
        rr.Points3D(
            [[0, 0, 0], [1, 0, 1]],
            # rr.Radius.ui_points produces radii that the viewer interprets as given in ui points.
            radii=rr.Radius.ui_points([40.0, 60.0]),
            colors=[255, 0, 0],
        ),
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/1200w.png">
      <img src="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/full.png" width="640">
    </picture>
    </center>

    ### Send several point clouds with varying point count over time in a single call:
    ```python
    from __future__ import annotations

    import numpy as np
    import rerun as rr

    rr.init("rerun_example_points3d_send_columns", spawn=True)

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

    rr.send_columns_v2(
        "points",
        indexes=[rr.TimeSecondsColumn("time", times)],
        columns=[
            *rr.Points3D.columns(positions=positions, _lengths=[2, 4, 4, 3, 4]),
            *rr.Points3D.columns(colors=colors, radii=radii),
        ],
    )
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/1200w.png">
      <img src="https://static.rerun.io/points3d_send_columns/633b524a2ee439b0e3afc3f894f4927ce938a3ec/full.png" width="640">
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
    def update_fields(
        cls,
        *,
        clear: bool = False,
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
        clear:
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

            if clear:
                kwargs = {k: v if v is not None else [] for k, v in kwargs.items()}  # type: ignore[misc]

            inst.__attrs_init__(**kwargs)
            return inst

        inst.__attrs_clear__()
        return inst

    @classmethod
    def clear_fields(cls) -> Points3D:
        """Clear all the fields of a `Points3D`."""
        inst = cls.__new__(cls)
        inst.__attrs_init__(
            positions=[],
            radii=[],
            colors=[],
            labels=[],
            show_labels=[],
            class_ids=[],
            keypoint_ids=[],
        )
        return inst

    @classmethod
    def columns(
        cls,
        *,
        _lengths: npt.ArrayLike | None = None,
        positions: datatypes.Vec3DArrayLike | None = None,
        radii: datatypes.Float32ArrayLike | None = None,
        colors: datatypes.Rgba32ArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        show_labels: datatypes.BoolArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
    ) -> list[ComponentColumn]:
        """
        Partitions the component data into multiple sub-batches.

        This makes it possible to use `rr.send_columns` to send columnar data directly into Rerun.

        If specified, `_lengths` must sum to the total length of the component batch.
        If left unspecified, it will default to unit-length batches.

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

        batches = [batch for batch in inst.as_component_batches() if isinstance(batch, DescribedComponentBatch)]
        if len(batches) == 0:
            return []

        if _lengths is None:
            _lengths = np.ones(len(batches[0]._batch.as_arrow_array()))

        columns = [batch.partition(_lengths) for batch in batches]

        indicator_batch = DescribedComponentBatch(cls.indicator(), cls.indicator().component_descriptor())
        indicator_column = indicator_batch.partition(np.zeros(len(_lengths)))  # type: ignore[arg-type]

        return [indicator_column] + columns

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
