# DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/python.rs:277.

from __future__ import annotations

from typing import Sequence, Union

import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .. import datatypes
from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import tensordata_init, tensordata_native_to_pa_array  # noqa: F401

__all__ = ["TensorData", "TensorDataArray", "TensorDataArrayLike", "TensorDataLike", "TensorDataType"]


def _tensordata_id_converter(x: datatypes.TensorIdLike) -> datatypes.TensorId:
    if isinstance(x, datatypes.TensorId):
        return x
    else:
        return datatypes.TensorId(x)


def _tensordata_buffer_converter(x: datatypes.TensorBufferLike) -> datatypes.TensorBuffer:
    if isinstance(x, datatypes.TensorBuffer):
        return x
    else:
        return datatypes.TensorBuffer(x)


@define(init=False)
class TensorData:
    """
    A multi-dimensional `Tensor` of data.

    The number of dimensions and their respective lengths is specified by the `shape` field.
    The dimensions are ordered from outermost to innermost. For example, in the common case of
    a 2D RGB Image, the shape would be `[height, width, channel]`.

    These dimensions are combined with an index to look up values from the `buffer` field,
    which stores a contiguous array of typed values.
    """

    def __init__(self, *args, **kwargs):  # type: ignore[no-untyped-def]
        tensordata_init(self, *args, **kwargs)

    id: datatypes.TensorId = field(converter=_tensordata_id_converter)
    shape: list[datatypes.TensorDimension] = field()
    buffer: datatypes.TensorBuffer = field(converter=_tensordata_buffer_converter)


TensorDataLike = TensorData
TensorDataArrayLike = Union[TensorData, Sequence[TensorDataLike], npt.ArrayLike]


# --- Arrow support ---


class TensorDataType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
            pa.struct(
                [
                    pa.field(
                        "id",
                        pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={}), 16),
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "shape",
                        pa.list_(
                            pa.field(
                                "item",
                                pa.struct(
                                    [
                                        pa.field("size", pa.uint64(), nullable=False, metadata={}),
                                        pa.field("name", pa.utf8(), nullable=True, metadata={}),
                                    ]
                                ),
                                nullable=False,
                                metadata={},
                            )
                        ),
                        nullable=False,
                        metadata={},
                    ),
                    pa.field(
                        "buffer",
                        pa.dense_union(
                            [
                                pa.field("_null_markers", pa.null(), nullable=True, metadata={}),
                                pa.field(
                                    "U8",
                                    pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "U16",
                                    pa.list_(pa.field("item", pa.uint16(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "U32",
                                    pa.list_(pa.field("item", pa.uint32(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "U64",
                                    pa.list_(pa.field("item", pa.uint64(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "I8",
                                    pa.list_(pa.field("item", pa.int8(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "I16",
                                    pa.list_(pa.field("item", pa.int16(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "I32",
                                    pa.list_(pa.field("item", pa.int32(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "I64",
                                    pa.list_(pa.field("item", pa.int64(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "F32",
                                    pa.list_(pa.field("item", pa.float32(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "F64",
                                    pa.list_(pa.field("item", pa.float64(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                                pa.field(
                                    "JPEG",
                                    pa.list_(pa.field("item", pa.uint8(), nullable=False, metadata={})),
                                    nullable=False,
                                    metadata={},
                                ),
                            ]
                        ),
                        nullable=False,
                        metadata={},
                    ),
                ]
            ),
            "rerun.datatypes.TensorData",
        )


class TensorDataArray(BaseExtensionArray[TensorDataArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.TensorData"
    _EXTENSION_TYPE = TensorDataType

    @staticmethod
    def _native_to_pa_array(data: TensorDataArrayLike, data_type: pa.DataType) -> pa.Array:
        return tensordata_native_to_pa_array(data, data_type)


TensorDataType._ARRAY_TYPE = TensorDataArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(TensorDataType())
