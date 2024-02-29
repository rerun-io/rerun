from __future__ import annotations

import numpy as np
import rerun as rr


def test_uuid() -> None:
    uuid_bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]

    # Single uuid.
    uuids = [uuid_bytes, np.array(uuid_bytes, dtype=np.uint8), np.array(uuid_bytes, dtype=np.uint32)]
    expected = rr.datatypes.UuidBatch([uuid_bytes]).as_arrow_array()
    for uuid in uuids:
        assert rr.datatypes.UuidBatch([uuid]).as_arrow_array() == expected

    # Several uuids.
    uuids_arrays = [
        [uuid_bytes, uuid_bytes],
        [np.array(uuid_bytes, dtype=np.uint8), uuid_bytes],
        [np.array(uuid_bytes, dtype=np.uint8), np.array(uuid_bytes, dtype=np.uint32)],
    ]
    expected = rr.datatypes.UuidBatch([uuid_bytes, uuid_bytes]).as_arrow_array()
    for uuids in uuids_arrays:
        assert rr.datatypes.UuidBatch(uuids).as_arrow_array() == expected
