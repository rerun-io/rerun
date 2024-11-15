from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

if TYPE_CHECKING:
    from . import ChannelDatatype


class ChannelDatatypeExt:
    """Extension for [ChannelDatatype][rerun.datatypes.ChannelDatatype]."""

    @staticmethod
    def from_np_dtype(dtype: Any) -> ChannelDatatype:
        from . import ChannelDatatype

        channel_datatype_from_np_dtype = {
            np.uint8: ChannelDatatype.U8,
            np.uint16: ChannelDatatype.U16,
            np.uint32: ChannelDatatype.U32,
            np.uint64: ChannelDatatype.U64,
            np.int8: ChannelDatatype.I8,
            np.int16: ChannelDatatype.I16,
            np.int32: ChannelDatatype.I32,
            np.int64: ChannelDatatype.I64,
            np.float16: ChannelDatatype.F16,
            np.float32: ChannelDatatype.F32,
            np.float64: ChannelDatatype.F64,
        }
        return channel_datatype_from_np_dtype[dtype.type]

    def to_np_dtype(self: Any) -> type:
        from . import ChannelDatatype

        channel_datatype_to_np_dtype = {
            ChannelDatatype.U8: np.uint8,
            ChannelDatatype.U16: np.uint16,
            ChannelDatatype.U32: np.uint32,
            ChannelDatatype.U64: np.uint64,
            ChannelDatatype.I8: np.int8,
            ChannelDatatype.I16: np.int16,
            ChannelDatatype.I32: np.int32,
            ChannelDatatype.I64: np.int64,
            ChannelDatatype.F16: np.float16,
            ChannelDatatype.F32: np.float32,
            ChannelDatatype.F64: np.float64,
        }
        return channel_datatype_to_np_dtype[self]
