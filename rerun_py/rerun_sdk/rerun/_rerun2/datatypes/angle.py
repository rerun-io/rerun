# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/angle.fbs".

# You can extend this class by creating a "AngleExt" class in "angle_ext.py".

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from .angle_ext import AngleExt

__all__ = ["Angle", "AngleArray", "AngleArrayLike", "AngleLike", "AngleType"]


@define(init=False)
class Angle(AngleExt):
    """Angle in either radians or degrees."""

    # __init__ can be found in angle_ext.py

    inner: float = field(converter=float)
    """
    Radians (float):
        3D rotation angle in radians. Only one of `degrees` or `radians` should be set.

    Degrees (float):
        3D rotation angle in degrees. Only one of `degrees` or `radians` should be set.
    """

    kind: Literal["radians", "degrees"] = field(default="radians")


if TYPE_CHECKING:
    AngleLike = Union[
        Angle,
        float,
    ]
    AngleArrayLike = Union[
        Angle,
        float,
        Sequence[AngleLike],
    ]
else:
    AngleLike = Any
    AngleArrayLike = Any

# --- Arrow support ---


class AngleType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.dense_union(
                [
                    pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                    pa.field("Radians", pa.float32(), nullable=False, metadata={}),
                    pa.field("Degrees", pa.float32(), nullable=False, metadata={}),
                ]
            ),
            "rerun.datatypes.Angle",
        )


class AngleArray(BaseExtensionArray[AngleArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.Angle"
    _EXTENSION_TYPE = AngleType

    @staticmethod
    def _native_to_pa_array(data: AngleArrayLike, data_type: pa.DataType) -> pa.Array:
        return AngleExt.native_to_pa_array_override(data, data_type)


AngleType._ARRAY_TYPE = AngleArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AngleType())
