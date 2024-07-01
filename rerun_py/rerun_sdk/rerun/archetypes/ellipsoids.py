# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/archetypes/ellipsoids.fbs".

# You can extend this class by creating a "EllipsoidsExt" class in "ellipsoids_ext.py".

from __future__ import annotations

from attrs import define, field

from .. import components
from .._baseclasses import (
    Archetype,
)
from .ellipsoids_ext import EllipsoidsExt

__all__ = ["Ellipsoids"]


@define(str=False, repr=False, init=False)
class Ellipsoids(EllipsoidsExt, Archetype):
    """
    **Archetype**: 3D ellipsoids with half-extents and optional center, rotations, rotations, colors etc.

    This archetype is for ellipsoids or spheres whose size is a key part of the data
    (e.g. a bounding sphere).
    For points whose radii are for the sake of visualization, use `Points3D` instead.

    Currently, ellipsoids are always rendered as wireframes.
    Opaque and transparent rendering will be supported later.
    """

    # __init__ can be found in ellipsoids_ext.py

    def __attrs_clear__(self) -> None:
        """Convenience method for calling `__attrs_init__` with all `None`s."""
        self.__attrs_init__(
            half_sizes=None,  # type: ignore[arg-type]
            centers=None,  # type: ignore[arg-type]
            rotations=None,  # type: ignore[arg-type]
            colors=None,  # type: ignore[arg-type]
            line_radii=None,  # type: ignore[arg-type]
            labels=None,  # type: ignore[arg-type]
            class_ids=None,  # type: ignore[arg-type]
        )

    @classmethod
    def _clear(cls) -> Ellipsoids:
        """Produce an empty Ellipsoids, bypassing `__init__`."""
        inst = cls.__new__(cls)
        inst.__attrs_clear__()
        return inst

    half_sizes: components.HalfSize3DBatch = field(
        metadata={"component": "required"},
        converter=components.HalfSize3DBatch._required,  # type: ignore[misc]
    )
    # For each ellipsoid, half of its size on its three axes.
    #
    # If all components are equal, then it is a sphere with that radius.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    centers: components.Position3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Position3DBatch._optional,  # type: ignore[misc]
    )
    # Optional center positions of the ellipsoids.
    #
    # If not specified, the centers will be at (0, 0, 0).
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    rotations: components.Rotation3DBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.Rotation3DBatch._optional,  # type: ignore[misc]
    )
    # Optional rotations of the boxes.
    #
    # If not specified, the axes of the ellipsoid align with the axes of the coordinate system.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    colors: components.ColorBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ColorBatch._optional,  # type: ignore[misc]
    )
    # Optional colors for the ellipsoids.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    line_radii: components.RadiusBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.RadiusBatch._optional,  # type: ignore[misc]
    )
    # Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    labels: components.TextBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.TextBatch._optional,  # type: ignore[misc]
    )
    # Optional text labels for the ellipsoids.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    class_ids: components.ClassIdBatch | None = field(
        metadata={"component": "optional"},
        default=None,
        converter=components.ClassIdBatch._optional,  # type: ignore[misc]
    )
    # Optional `ClassId`s for the ellipsoids.
    #
    # The class ID provides colors and labels if not specified explicitly.
    #
    # (Docstring intentionally commented out to hide this field from the docs)

    __str__ = Archetype.__str__
    __repr__ = Archetype.__repr__  # type: ignore[assignment]
