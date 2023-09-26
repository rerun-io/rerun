# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/origin3d.fbs".

# You can extend this class by creating a "Origin3DExt" class in "origin3d_ext.py".

from __future__ import annotations

from typing import Any

import numpy.typing as npt

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Origin3D", "Origin3DBatch", "Origin3DType"]


class Origin3D(datatypes.Vec3D):
    """A point of origin in 3D space."""

    def __init__(self: Any, xyz: npt.ArrayLike):
        # You can define your own __init__ function as a member of Origin3DExt in origin3d_ext.py
        self.__attrs_init__(xyz=xyz)

    # Note: there are no fields here because Origin3D delegates to datatypes.Vec3D


class Origin3DType(datatypes.Vec3DType):
    _TYPE_NAME: str = "rerun.components.Origin3D"


class Origin3DBatch(datatypes.Vec3DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Origin3DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Origin3DType())
