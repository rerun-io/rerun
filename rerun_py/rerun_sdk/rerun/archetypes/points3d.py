# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs".

# You can extend this class by creating a "Points3DExt" class in "points3d_ext.py".

from __future__ import annotations

from typing import Any

from attrs import define, field

from .. import components, datatypes
from .._baseclasses import Archetype
from ..error_utils import catch_and_log_exceptions

__all__ = ["Points3D"]


@define(str=False, repr=False, init=False)
class Points3D(Archetype):
    """
    A 3D point cloud with positions and optional colors, radii, labels, etc.

    Examples
    --------
    ```python
    import rerun as rr

    rr.init("rerun_example_points3d_simple", spawn=True)

    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
    ```
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1200w.png">
      <img src="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/full.png">
    </picture>

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
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1200w.png">
      <img src="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/full.png">
    </picture>
    """

    @catch_and_log_exceptions()
    def __init__(
        self: Any,
        positions: datatypes.Vec3DArrayLike,
        *,
        radii: components.RadiusArrayLike | None = None,
        colors: datatypes.ColorArrayLike | None = None,
        labels: datatypes.Utf8ArrayLike | None = None,
        class_ids: datatypes.ClassIdArrayLike | None = None,
        keypoint_ids: datatypes.KeypointIdArrayLike | None = None,
        instance_keys: components.InstanceKeyArrayLike | None = None,
    ):
        """
        Create a new instance of the Points3D archetype.

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
        class_ids:
             Optional class Ids for the points.

             The class ID provides colors and labels if not specified explicitly.
        keypoint_ids:
             Optional keypoint IDs for the points, identifying them within a class.

             If keypoint IDs are passed in but no class IDs were specified, the class ID will
             default to 0.
             This is useful to identify points within a single classification (which is identified
             with `class_id`).
             E.g. the classification might be 'Person' and the keypoints refer to joints on a
             detected skeleton.
        instance_keys:
             Unique identifiers for each individual point in the batch.
        """

        # You can define your own __init__ function as a member of Points3DExt in points3d_ext.py
        self.__attrs_init__(
            positions=positions,
            radii=radii,
            colors=colors,
            labels=labels,
            class_ids=class_ids,
            keypoint_ids=keypoint_ids,
            instance_keys=instance_keys,
        )

    positions: components.Position3DBatch = field(
        metadata={"component": "required"},
        converter=components.Position3DBatch,  # type: ignore[misc]
    )
    """
    All the 3D positions at which the point cloud shows points.
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
