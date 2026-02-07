from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from rerun.datatypes.uint16 import UInt16
    from rerun.datatypes.uint64 import UInt64

    from . import (
        ChannelCountPairArrayLike,
    )


class ChannelCountPairExt:
    """Extension for [ChannelCountPair][rerun.datatypes.ChannelCountPair]."""

    @staticmethod
    def native_to_pa_array_override(data: ChannelCountPairArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import ChannelCountPair, UInt16Batch, UInt64Batch

        if isinstance(data, ChannelCountPair):
            channel_id_batch = UInt16Batch(data.channel_id)
            message_count_batch = UInt64Batch(data.message_count)
        else:
            # non-numpy Sequence[ChannelCountPair | Tuple(UInt16, UInt64)]
            channel_ids: list[UInt16 | int] = []
            message_counts: list[UInt64 | int] = []
            for item in data:
                if isinstance(item, ChannelCountPair):
                    channel_ids.append(item.channel_id)
                    message_counts.append(item.message_count)
                else:
                    channel_ids.append(item[0])
                    message_counts.append(item[1])
            channel_id_batch = UInt16Batch(channel_ids)
            message_count_batch = UInt64Batch(message_counts)

        return pa.StructArray.from_arrays(
            arrays=[channel_id_batch.as_arrow_array(), message_count_batch.as_arrow_array()],
            fields=[data_type.field("channel_id"), data_type.field("message_count")],
        )
