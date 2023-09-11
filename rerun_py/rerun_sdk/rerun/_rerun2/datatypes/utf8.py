# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs
# Based on "crates/re_types/definitions/rerun/datatypes/utf8.fbs".


from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence, Union

import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import override_utf8___native_to_pa_array_override  # noqa: F401

__all__ = ["Utf8", "Utf8Array", "Utf8ArrayLike", "Utf8Like", "Utf8Type"]


@define
class Utf8:
    """A string of text, encoded as UTF-8."""

    # You can define your own __init__ function by defining a function called {init_override_name:?}

    value: str = field(converter=str)

    def __str__(self) -> str:
        return str(self.value)


if TYPE_CHECKING:
    Utf8Like = Union[Utf8, str]
else:
    Utf8Like = Any

Utf8ArrayLike = Union[Utf8, Sequence[Utf8Like], str, Sequence[str]]


# --- Arrow support ---


class Utf8Type(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(self, pa.utf8(), "rerun.label")


class Utf8Array(BaseExtensionArray[Utf8ArrayLike]):
    _EXTENSION_NAME = "rerun.label"
    _EXTENSION_TYPE = Utf8Type

    @staticmethod
    def _native_to_pa_array(data: Utf8ArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "override_utf8__native_to_pa_array_override" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/utf8.py


Utf8Type._ARRAY_TYPE = Utf8Array

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(Utf8Type())
