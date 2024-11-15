from __future__ import annotations

import numpy as np
import rerun as rr


def test_blob() -> None:
    """Blob should accept bytes input."""

    bytes = b"Hello world"
    array = np.array([72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100], dtype=np.uint8)

    assert rr.datatypes.BlobBatch(bytes).as_arrow_array() == rr.datatypes.BlobBatch(array).as_arrow_array()


def test_blob_arrays() -> None:
    COUNT = 10

    # bytes & array
    bytes = [b"Hello world"] * COUNT
    array = [np.array([72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100], dtype=np.uint8)] * COUNT
    assert rr.datatypes.BlobBatch(bytes).as_arrow_array() == rr.datatypes.BlobBatch(array).as_arrow_array()
    assert len(rr.datatypes.BlobBatch(bytes)) == COUNT
    assert len(rr.datatypes.BlobBatch(array)) == COUNT

    # 2D numpy array
    array_2d = np.array([[72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100]] * COUNT, dtype=np.uint8)
    assert rr.datatypes.BlobBatch(bytes).as_arrow_array() == rr.datatypes.BlobBatch(array_2d).as_arrow_array()
    assert len(rr.datatypes.BlobBatch(array_2d)) == COUNT
