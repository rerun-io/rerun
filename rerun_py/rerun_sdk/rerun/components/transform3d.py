# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/components/transform3d.fbs".

# You can extend this class by creating a "Transform3DExt" class in "transform3d_ext.py".

from __future__ import annotations

from .. import datatypes
from .._baseclasses import ComponentBatchMixin

__all__ = ["Transform3D", "Transform3DBatch", "Transform3DType"]


class Transform3D(datatypes.Transform3D):
    """An affine transform between two 3D spaces, represented in a given direction."""

    # Note: there are no fields here because Transform3D delegates to datatypes.Transform3D
    pass


class Transform3DType(datatypes.Transform3DType):
    _TYPE_NAME: str = "rerun.components.Transform3D"


class Transform3DBatch(datatypes.Transform3DBatch, ComponentBatchMixin):
    _ARROW_TYPE = Transform3DType()


# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Transform3DType())
