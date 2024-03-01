from __future__ import annotations

import numpy as np
import rerun as rr

from tests.unit.common_arrays import uuid_bytes0, uuids_arrays, uuids_expected


def test_uuid() -> None:
    # Single uuid.
    uuids = [uuid_bytes0, np.array(uuid_bytes0, dtype=np.uint8), np.array(uuid_bytes0, dtype=np.uint32)]
    expected = rr.datatypes.UuidBatch([uuid_bytes0]).as_arrow_array()
    for uuid in uuids:
        assert rr.datatypes.UuidBatch([uuid]).as_arrow_array() == expected

    # Several uuids.
    for uuids in uuids_arrays:
        expected = uuids_expected(uuids)
        assert rr.datatypes.UuidBatch(uuids).as_arrow_array() == expected.as_arrow_array()
