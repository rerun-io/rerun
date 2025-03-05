# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/datatypes/utf8.fbs".

# You can extend this class by creating a "Utf8Ext" class in "utf8_ext.py".

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseBatch,
)

__all__ = ["Utf8", "Utf8ArrayLike", "Utf8Batch", "Utf8Like"]


@define(init=False)
class Utf8:
    """**Datatype**: A string of text, encoded as UTF-8."""

    def __init__(self: Any, value: Utf8Like) -> None:
        """Create a new instance of the Utf8 datatype."""

        # You can define your own __init__ function as a member of Utf8Ext in utf8_ext.py
        self.__attrs_init__(value=value)

    value: str = field(
        converter=str,
    )

    def __str__(self) -> str:
        return str(self.value)

    def __hash__(self) -> int:
        return hash(self.value)


if TYPE_CHECKING:
    Utf8Like = Union[
        Utf8,
        str,
    ]
else:
    Utf8Like = Any

Utf8ArrayLike = Union[
    Utf8,
    Sequence[Utf8Like],
    str,
    Sequence[str],
    npt.ArrayLike,
]


class Utf8Batch(BaseBatch[Utf8ArrayLike]):
    _ARROW_DATATYPE = pa.utf8()

    @staticmethod
    def _native_to_pa_array(data: Utf8ArrayLike, data_type: pa.DataType) -> pa.Array:
        if isinstance(data, str):
            array: list[str] | npt.ArrayLike = [data]
        elif isinstance(data, Sequence):
            array = [str(datum) for datum in data]
        elif isinstance(data, np.ndarray):
            array = data
        else:
            array = [str(data)]

        return pa.array(array, type=data_type)
