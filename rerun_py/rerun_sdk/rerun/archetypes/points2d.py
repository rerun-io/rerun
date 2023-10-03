# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/points2d.fbs".

# You can extend this class by creating a "Points2DExt" class in "points2d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from .points2d_ext import Points2DExt

__all__ = ["Points2D"]


@define(str=False, repr=False, init=False)
class Points2D(Points2DExt, Archetype):
    """
    A 2D point cloud with positions and optional colors, radii, labels, etc.

    Examples
    --------
    ```python
    import rerun as rr

    rr.init("rerun_example_points2d", spawn=True)

    rr.log("points", rr.Points2D([[0, 0], [1, 1]]))

    # Log an extra rect to set the view bounds
    rr.log("bounds", rr.Boxes2D(half_sizes=[2, 1.5]))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/1200w.png">
      <img src="https://static.rerun.io/point2d_simple/a8e801958bce5aa4e080659c033630f86ce95f71/full.png">
    </picture>

    ```python
    import rerun as rr
    from numpy.random import default_rng

    rr.init("rerun_example_points2d", spawn=True)
    rng = default_rng(12345)

    positions = rng.uniform(-3, 3, size=[10, 2])
    colors = rng.uniform(0, 255, size=[10, 4])
    radii = rng.uniform(0, 1, size=[10])

    rr.log("random", rr.Points2D(positions, colors=colors, radii=radii))

    # Log an extra rect to set the view bounds
    rr.log("bounds", rr.Boxes2D(half_sizes=[4, 3]))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1200w.png">
      <img src="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/full.png">
    </picture>
    """

    # __init__ can be found in points2d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            positions=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            draw_order=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
            keypoint_ids=None,  # type: ignore[arg-type]
            instance_keys=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Points2D:
        """Produce an empty Points2D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    positions: components.Position2DBatch = field(
        metadata={"component": "required"},
        converter=components.Position2DBatch._required,  # type: ignore[misc]
    )
    """
    All the 2D positions at which the point cloud shows points.
    """

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    """
    Optional radii for the points, effectively turning them into circles.
    """

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    """
    Optional colors for the points.

    The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    As either 0-1 floats or 0-255 integers, with separate alpha.
    """

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    """
    Optional text labels for the points.
    """

    draw_order: components.DrawOrderBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.DrawOrderBatch._optional,  # type: ignore[misc]
    )
    """
    An optional floating point value that specifies the 2D drawing order.

    Objects with higher values are drawn on top of those with lower values.
    """

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    """
    Optional class Ids for the points.

    The class ID provides colors and labels if not specified explicitly.
    """

    keypoint_ids: components.KeypointIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.KeypointIdBatch._optional,  # type: ignore[misc]
    )
    """
    Optional keypoint IDs for the points, identifying them within a class.

    If keypoint IDs are passed in but no class IDs were specified, the class ID will
    default to 0.
    This is useful to identify points within a single classification (which is identified
    with `class_id`).
    E.g. the classification might be 'Person' and the keypoints refer to joints on a
    detected skeleton.
    """

    instance_keys: components.InstanceKeyBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyBatch._optional,  # type: ignore[misc]
    )
    """
    Unique identifiers for each individual point in the batch.
    """

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__


if hasattr(Points2DExt, "deferred_patch_class"):
    Points2DExt.deferred_patch_class(Points2D)
