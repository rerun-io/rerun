# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/half_size3d.fbs".

# You can extend this class by creating a "HalfSize3DExt" class in "half_size3d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["HalfSize3D", "HalfSize3DBatch"]


class HalfSize3D(datatypes.Vec3D, ComponentMixin):
    """
    **Component**: Half-size (radius) of a 3D box.

    Measured in its local coordinate system.

    The box extends both in negative and positive direction along each axis.
    Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
    """

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of HalfSize3DExt in half_size3d_ext.py

    # Note: there are no fields here because HalfSize3D delegates to datatypes.Vec3D
    pass


class HalfSize3DBatch(datatypes.Vec3DBatch, ComponentBatchMixin):
    _COMPONENT_NAME: str = "rerun.components.HalfSize3D"


# This is patched in late to avoid circular dependencies.
HalfSize3D._BATCH_TYPE = HalfSize3DBatch  # type: ignore[assignment]
