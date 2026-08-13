from __future__ import annotations

import numpy as np
from rerun import encodings


def test_utf8_batch_single() -> None:
    single_string = "hello"
    list_of_one_string = ["hello"]
    array_of_one_string = np.array(["hello"])

    assert (
        encodings.Utf8Batch(single_string).as_arrow_array() == encodings.Utf8Batch(list_of_one_string).as_arrow_array()
    )

    assert (
        encodings.Utf8Batch(single_string).as_arrow_array() == encodings.Utf8Batch(array_of_one_string).as_arrow_array()
    )


def test_utf8_batch_many() -> None:
    # different string length to be sure
    list_of_strings = ["hell", "worlds"]
    array_of_strings = np.array(["hell", "worlds"])

    assert (
        encodings.Utf8Batch(list_of_strings).as_arrow_array() == encodings.Utf8Batch(array_of_strings).as_arrow_array()
    )
