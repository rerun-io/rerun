# DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/python.rs.

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Literal, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from attrs import define, field

from .._baseclasses import (
    BaseExtensionArray,
    BaseExtensionType,
)
from ._overrides import tensorbuffer_inner_converter  # noqa: F401

__all__ = ["TensorBuffer", "TensorBufferArray", "TensorBufferArrayLike", "TensorBufferLike", "TensorBufferType"]


@define
class TensorBuffer:
    """
    The underlying storage for a `Tensor`.

    Tensor elements are stored in a contiguous buffer of a single type.
    """

    inner: npt.NDArray[np.float32] | npt.NDArray[np.float64] | npt.NDArray[np.int16] | npt.NDArray[
        np.int32
    ] | npt.NDArray[np.int64] | npt.NDArray[np.int8] | npt.NDArray[np.uint16] | npt.NDArray[np.uint32] | npt.NDArray[
        np.uint64
    ] | npt.NDArray[
        np.uint8
    ] = field(
        converter=tensorbuffer_inner_converter
    )
    """
    U8 (npt.NDArray[np.uint8]):

    U16 (npt.NDArray[np.uint16]):

    U32 (npt.NDArray[np.uint32]):

    U64 (npt.NDArray[np.uint64]):

    I8 (npt.NDArray[np.int8]):

    I16 (npt.NDArray[np.int16]):

    I32 (npt.NDArray[np.int32]):

    I64 (npt.NDArray[np.int64]):

    F32 (npt.NDArray[np.float32]):

    F64 (npt.NDArray[np.float64]):

    JPEG (npt.NDArray[np.uint8]):
    """

    kind: Literal["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "f32", "f64", "jpeg"] = field(default="u8")


if TYPE_CHECKING:
    TensorBufferLike = Union[
        TensorBuffer,
        npt.NDArray[np.float32],
        npt.NDArray[np.float64],
        npt.NDArray[np.int16],
        npt.NDArray[np.int32],
        npt.NDArray[np.int64],
        npt.NDArray[np.int8],
        npt.NDArray[np.uint16],
        npt.NDArray[np.uint32],
        npt.NDArray[np.uint64],
        npt.NDArray[np.uint8],
    ]
    TensorBufferArrayLike = Union[
        TensorBuffer,
        npt.NDArray[np.float32],
        npt.NDArray[np.float64],
        npt.NDArray[np.int16],
        npt.NDArray[np.int32],
        npt.NDArray[np.int64],
        npt.NDArray[np.int8],
        npt.NDArray[np.uint16],
        npt.NDArray[np.uint32],
        npt.NDArray[np.uint64],
        npt.NDArray[np.uint8],
        Sequence[TensorBufferLike],
    ]
else:
    TensorBufferLike = Any
    TensorBufferArrayLike = Any

# --- Arrow support ---


class TensorBufferType(BaseExtensionType):
    def __init__(self) -> None:
        pa.ExtensionType.__init__(
            self,
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
            "rerun.datatypes.TensorBuffer",
        )


class TensorBufferArray(BaseExtensionArray[TensorBufferArrayLike]):
    _EXTENSION_NAME = "rerun.datatypes.TensorBuffer"
    _EXTENSION_TYPE = TensorBufferType

    @staticmethod
    def _native_to_pa_array(data: TensorBufferArrayLike, data_type: pa.DataType) -> pa.Array:
        raise NotImplementedError  # You need to implement "tensorbuffer_native_to_pa_array" in rerun_py/rerun_sdk/rerun/_rerun2/datatypes/_overrides/tensor_buffer.py


TensorBufferType._ARRAY_TYPE = TensorBufferArray

# TODO(cmc): bring back registration to pyarrow once legacy types are gone
# pa.register_extension_type(TensorBufferType())
