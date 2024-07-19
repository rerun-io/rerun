from __future__ import annotations

from typing import TYPE_CHECKING, Any

import numpy as np

if TYPE_CHECKING:
    from . import ChannelDatatype


class ChannelDatatypeExt:
    """Extension for [ChannelDatatype][rerun.components.ChannelDatatype]."""

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
