from __future__ import annotations

import numpy as np
from rerun import datatypes


def test_utf8_batch_single() -> None:
    single_string = "hello"
    list_of_one_string = ["hello"]
    array_of_one_string = np.array(["hello"])

    assert (
        datatypes.Utf8Batch(single_string).as_arrow_array() == datatypes.Utf8Batch(list_of_one_string).as_arrow_array()
    )

    assert (
        datatypes.Utf8Batch(single_string).as_arrow_array() == datatypes.Utf8Batch(array_of_one_string).as_arrow_array()
    )


def test_utf8_batch_many() -> None:
    # different string length to be sure
    list_of_strings = ["hell", "worlds"]
    array_of_strings = np.array(["hell", "worlds"])

    assert (
        datatypes.Utf8Batch(list_of_strings).as_arrow_array() == datatypes.Utf8Batch(array_of_strings).as_arrow_array()
    )
