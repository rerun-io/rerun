# NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

from __future__ import annotations

from typing import (Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union,
    TYPE_CHECKING, SupportsFloat, Literal)

from attrs import define, field
import numpy as np
import numpy.typing as npt
import pyarrow as pa

from .._baseclasses import (
    Archetype,
    BaseExtensionType,
    BaseExtensionArray,
    BaseDelegatingExtensionType,
    BaseDelegatingExtensionArray
)
from .._converters import (
    int_or_none,
    float_or_none,
    bool_or_none,
    str_or_none,
    to_np_uint8,
    to_np_uint16,
    to_np_uint32,
    to_np_uint64,
    to_np_int8,
    to_np_int16,
    to_np_int32,
    to_np_int64,
    to_np_bool,
    to_np_float16,
    to_np_float32,
    to_np_float64
)
from .. import datatypes
__all__ = ["ColorArray", "ColorType"]


class ColorType(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.colorrgba"
    _DELEGATED_EXTENSION_TYPE = datatypes.ColorType

class ColorArray(BaseDelegatingExtensionArray[datatypes.ColorArrayLike]):
    _EXTENSION_NAME = "rerun.colorrgba"
    _EXTENSION_TYPE = ColorType
    _DELEGATED_ARRAY_TYPE = datatypes.ColorArray

ColorType._ARRAY_TYPE = ColorArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(ColorType())


