# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/re_types/definitions/rerun/archetypes/points3d.fbs".

# You can extend this class by creating a "Points3DExt" class in "points3d_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import Archetype
from .points3d_ext import Points3DExt

__all__ = ["Points3D"]


@define(str=False, repr=False, init=False)
class Points3D(Points3DExt, Archetype):
    """
    **Archetype**: A 3D point cloud with positions and optional colors, radii, labels, etc.

    Example
    -------
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

    """

    # __init__ can be found in points3d_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            positions=None,  # type: ignore[arg-type]
            radii=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
            keypoint_ids=None,  # type: ignore[arg-type]
            instance_keys=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Points3D:
        """Produce an empty Points3D, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    positions: components.Position3DBatch = field(
        metadata={"component": "required"},
        converter=components.Position3DBatch._required,  # type: ignore[misc]
    )
    # All the 3D positions at which the point cloud shows points.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for the points, effectively turning them into circles.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the points.
    #
    # The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    # As either 0-1 floats or 0-255 integers, with separate alpha.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the points.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional class Ids for the points.
    #
    # The class ID provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    keypoint_ids: components.KeypointIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.KeypointIdBatch._optional,  # type: ignore[misc]
    )
    # Optional keypoint IDs for the points, identifying them within a class.
    #
    # If keypoint IDs are passed in but no class IDs were specified, the class ID will
    # default to 0.
    # This is useful to identify points within a single classification (which is identified
    # with `class_id`).
    # E.g. the classification might be 'Person' and the keypoints refer to joints on a
    # detected skeleton.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    instance_keys: components.InstanceKeyBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.InstanceKeyBatch._optional,  # type: ignore[misc]
    )
    # Unique identifiers for each individual point in the batch.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
