from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from . import (
        KeypointPair,
        KeypointPairArrayLike,
        KeypointPairLike,
    )


def _keypoint_pair_converter(
    data: KeypointPairLike,
) -> KeypointPair:
    from . import KeypointPair

    if isinstance(data, KeypointPair):
        return data
    else:
        return KeypointPair(*data)


class KeypointPairExt:
    """Extension for [KeypointPair][rerun.datatypes.KeypointPair]."""

    @staticmethod
    def native_to_pa_array_override(data: KeypointPairArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import KeypointIdBatch, KeypointPair

        if isinstance(data, KeypointPair):
            data = [data]

        keypoints = [_keypoint_pair_converter(item) for item in data]

        keypoint0 = [pair.keypoint0 for pair in keypoints]
        keypoint1 = [pair.keypoint1 for pair in keypoints]

        keypoint0_array = KeypointIdBatch(keypoint0).as_arrow_array()
        keypoint1_array = KeypointIdBatch(keypoint1).as_arrow_array()

        return pa.StructArray.from_arrays(
            arrays=[keypoint0_array, keypoint1_array],
            fields=[data_type.field("keypoint0"), data_type.field("keypoint1")],
        )
