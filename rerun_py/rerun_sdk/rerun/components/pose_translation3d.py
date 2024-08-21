# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/components/translation3d.fbs".

# You can extend this class by creating a "PoseTranslation3DExt" class in "pose_translation3d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    ComponentBatchMixin,
    ComponentMixin,
)

__all__ = ["PoseTranslation3D", "PoseTranslation3DBatch", "PoseTranslation3DType"]


class PoseTranslation3D(datatypes.Vec3D, ComponentMixin):
    """**Component**: A translation vector in 3D space that doesn't propagate in the transform hierarchy."""

    _BATCH_TYPE = None
    # You can define your own __init__ function as a member of PoseTranslation3DExt in pose_translation3d_ext.py

    # Note: there are no fields here because PoseTranslation3D delegates to datatypes.Vec3D
    pass


class PoseTranslation3DType(datatypes.Vec3DType):
    _TYPE_NAME: str = "rerun.components.PoseTranslation3D"


class PoseTranslation3DBatch(datatypes.Vec3DBatch, ComponentBatchMixin):
    _ARROW_TYPE = PoseTranslation3DType()


# This is patched in late to avoid circular dependencies.
PoseTranslation3D._BATCH_TYPE = PoseTranslation3DBatch  # type: ignore[assignment]
