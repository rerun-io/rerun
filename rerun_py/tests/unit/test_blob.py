from __future__ import annotations

import numpy as np
import rerun as rr


def test_blob() -> None:
    """Blob should accept bytes input."""

    bites = b"Hello world"
    array = np.array([72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100], dtype=np.uint8)

    assert rr.components.BlobBatch(bites).as_arrow_array() == rr.components.BlobBatch(array).as_arrow_array()
