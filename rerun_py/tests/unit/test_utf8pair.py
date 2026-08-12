from __future__ import annotations

import numpy as np
from rerun import encodings


def test_utf8pair_batch_single() -> None:
    single_pair_batches = [
        encodings.Utf8PairBatch(encodings.Utf8Pair("one", "two")),
        encodings.Utf8PairBatch([("one", "two")]),
        encodings.Utf8PairBatch([("one", encodings.Utf8("two"))]),
        encodings.Utf8PairBatch([(encodings.Utf8("one"), encodings.Utf8("two"))]),
        encodings.Utf8PairBatch([(encodings.Utf8("one"), "two")]),
        encodings.Utf8PairBatch(np.array([["one", "two"]])),
    ]

    for batch in single_pair_batches[1:]:
        assert single_pair_batches[0].as_arrow_array() == batch.as_arrow_array()


def test_utf8pair_batch_multiple() -> None:
    multiple_pair_batches = [
        encodings.Utf8PairBatch([encodings.Utf8Pair("one", "two"), encodings.Utf8Pair("three", "four")]),
        encodings.Utf8PairBatch([("one", "two"), ("three", "four")]),
        encodings.Utf8PairBatch(np.array([("one", "two"), ("three", "four")])),
    ]

    for batch in multiple_pair_batches[1:]:
        assert multiple_pair_batches[0].as_arrow_array() == batch.as_arrow_array()
