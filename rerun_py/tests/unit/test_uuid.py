from __future__ import annotations

from typing import Any

import numpy as np
import rerun as rr
from rerun.datatypes.uuid import UuidBatch

from .common_arrays import none_empty_or_value, uuid_bytes0, uuids_arrays


def uuids_expected(obj: Any) -> Any:
    expected = none_empty_or_value(obj, uuids_arrays[-1])
    return UuidBatch._optional(expected)


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
