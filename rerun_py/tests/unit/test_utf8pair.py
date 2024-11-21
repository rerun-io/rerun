from __future__ import annotations

import numpy as np
from rerun import datatypes


def test_utf8pair_batch_single() -> None:
    single_pair_batches = [
        datatypes.Utf8PairBatch(datatypes.Utf8Pair("one", "two")),
        datatypes.Utf8PairBatch([("one", "two")]),
        datatypes.Utf8PairBatch([("one", datatypes.Utf8("two"))]),
        datatypes.Utf8PairBatch([(datatypes.Utf8("one"), datatypes.Utf8("two"))]),
        datatypes.Utf8PairBatch([(datatypes.Utf8("one"), "two")]),
        datatypes.Utf8PairBatch(np.array([["one", "two"]])),
    ]

    for batch in single_pair_batches[1:]:
        assert single_pair_batches[0].as_arrow_array() == batch.as_arrow_array()


def test_utf8pair_batch_multiple() -> None:
    multiple_pair_batches = [
        datatypes.Utf8PairBatch([datatypes.Utf8Pair("one", "two"), datatypes.Utf8Pair("three", "four")]),
        datatypes.Utf8PairBatch([("one", "two"), ("three", "four")]),
        datatypes.Utf8PairBatch(np.array([("one", "two"), ("three", "four")])),
    ]

    for batch in multiple_pair_batches[1:]:
        assert multiple_pair_batches[0].as_arrow_array() == batch.as_arrow_array()
