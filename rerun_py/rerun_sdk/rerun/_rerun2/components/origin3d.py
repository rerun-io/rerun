# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/origin3d.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["Origin3D", "Origin3DArray", "Origin3DType"]


class Origin3D(datatypes.Vec3D):
    """A point of origin in 3D space."""

    # You can define your own __init__ function as a member of Origin3DExt in origin3d_ext.py

    # Note: there are no fields here because Origin3D delegates to datatypes.Vec3D


class Origin3DType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.Origin3D"
    _DELEGATED_EXTENSION_TYPE = datatypes.Vec3DType


class Origin3DArray(BaseDelegatingExtensionArray[datatypes.Vec3DArrayLike]):
    _EXTENSION_NAME = "rerun.components.Origin3D"
    _EXTENSION_TYPE = Origin3DType
    _DELEGATED_ARRAY_TYPE = datatypes.Vec3DArray


Origin3DType._ARRAY_TYPE = Origin3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Origin3DType())
