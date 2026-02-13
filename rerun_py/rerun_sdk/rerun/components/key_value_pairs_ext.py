from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

import pyarrow as pa

if TYPE_CHECKING:
    from ..datatypes import Utf8PairLike
    from . import KeyValuePairsArrayLike


from ..datatypes.utf8pair_ext import _utf8_pair_converter


class KeyValuePairsExt:
    """Extension for [KeyValuePairs][rerun.components.KeyValuePairs]."""

    @staticmethod
    def pairs__field_converter_override(data: Sequence[Utf8PairLike]) -> list[Utf8PairLike]:
        if isinstance(data, dict):
            return [_utf8_pair_converter((k, v)) for k, v in data.items()]
        elif hasattr(data, "__iter__") and not isinstance(data, str):
            pairs = []
            for item in data:
                if hasattr(item, "first") and hasattr(item, "second"):
                    pairs.append(item)
                elif isinstance(item, dict):
                    pairs.extend([_utf8_pair_converter((k, v)) for k, v in item.items()])
                else:
                    # Use the converter for tuple-like items
                    pairs.append(_utf8_pair_converter(item))
            return pairs
        else:
            raise ValueError(f"Cannot convert {type(data)} to `KeyValuePairs`")

    @staticmethod
    def native_to_pa_array_override(data: KeyValuePairsArrayLike, data_type: pa.DataType) -> pa.Array:
        from ..datatypes import Utf8PairBatch
        from . import KeyValuePairs

        # Handle single dict case - convert to KeyValuePairs
        if isinstance(data, dict):
            data = KeyValuePairs(data)

        # Handle single KeyValuePairs instance
        if isinstance(data, KeyValuePairs):
            inner_array = Utf8PairBatch(data.pairs).as_arrow_array()
            return pa.ListArray.from_arrays([0, len(inner_array)], inner_array, type=data_type)

        # Handle sequence of KeyValuePairs or dicts
        if isinstance(data, Sequence):
            all_pairs = []
            offsets = [0]

            for item in data:
                if isinstance(item, dict):
                    pairs = [_utf8_pair_converter((k, v)) for k, v in item.items()]
                elif isinstance(item, KeyValuePairs):
                    pairs = item.pairs
                else:
                    # Try to treat as KeyValuePairs-like
                    try:
                        kv_pairs = KeyValuePairs(item)
                        pairs = kv_pairs.pairs
                    except Exception as err:
                        raise ValueError(f"Cannot convert {type(item)} to `KeyValuePairs`") from err

                all_pairs.extend(pairs)
                offsets.append(len(all_pairs))

            if len(all_pairs) == 0:
                inner_array = Utf8PairBatch([]).as_arrow_array()
            else:
                inner_array = Utf8PairBatch(all_pairs).as_arrow_array()

            return pa.ListArray.from_arrays(offsets, inner_array, type=data_type)

        # Fallback, try to convert to KeyValuePairs
        try:
            kv_pairs = KeyValuePairs(data)
            inner_array = Utf8PairBatch(kv_pairs.pairs).as_arrow_array()
            return pa.ListArray.from_arrays([0, len(inner_array)], inner_array, type=data_type)
        except Exception as err:
            raise ValueError(f"Cannot convert {type(data)} to `KeyValuePairs`") from err
