# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/point3d.fbs".


from __future__ import annotations

from .. import datatypes
from .._baseclasses import (
    BaseDelegatingExtensionArray,
    BaseDelegatingExtensionType,
)

__all__ = ["Point3DArray", "Point3DType"]


class Point3DType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.components.Point3D"
    _DELEGATED_EXTENSION_TYPE = datatypes.Vec3DType


class Point3DArray(BaseDelegatingExtensionArray[datatypes.Vec3DArrayLike]):
    _EXTENSION_NAME = "rerun.components.Point3D"
    _EXTENSION_TYPE = Point3DType
    _DELEGATED_ARRAY_TYPE = datatypes.Vec3DArray


Point3DType._ARRAY_TYPE = Point3DArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Point3DType())
