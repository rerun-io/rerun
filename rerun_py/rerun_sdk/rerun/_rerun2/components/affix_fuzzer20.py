# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".



from __future__ import annotations

from typing import (Any, Dict, Iterable, Optional, Sequence, Set, Tuple, Union,
    TYPE_CHECKING, SupportsFloat, Literal)

from attrs import define, field
import numpy as np
import numpy.typing as npt
import pyarrow as pa
import uuid

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
__all__ = ["AffixFuzzer20", "AffixFuzzer20Array", "AffixFuzzer20Type"]


class AffixFuzzer20(datatypes.AffixFuzzer20):
    # You can define your own __init__ function as a member of AffixFuzzer20Ext in affix_fuzzer20_ext.py

    # Note: there are no fields here because AffixFuzzer20 delegates to datatypes.AffixFuzzer20


class AffixFuzzer20Type(BaseDelegatingExtensionType):
    _TYPE_NAME = "rerun.testing.components.AffixFuzzer20"
    _DELEGATED_EXTENSION_TYPE = datatypes.AffixFuzzer20Type

class AffixFuzzer20Array(BaseDelegatingExtensionArray[datatypes.AffixFuzzer20ArrayLike]):
    _EXTENSION_NAME = "rerun.testing.components.AffixFuzzer20"
    _EXTENSION_TYPE = AffixFuzzer20Type
    _DELEGATED_ARRAY_TYPE = datatypes.AffixFuzzer20Array

AffixFuzzer20Type._ARRAY_TYPE = AffixFuzzer20Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(AffixFuzzer20Type())


