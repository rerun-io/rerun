from __future__ import annotations

import uuid

import numpy as np
import numpy.typing as npt


def tensorid_uuid_converter(id: bytes | uuid.UUID | npt.ArrayLike) -> npt.NDArray[np.uint8]:
    if isinstance(id, uuid.UUID):
        id = id.bytes

    if isinstance(id, bytes):
        id = list(id)

    id = np.asarray(id, dtype=np.uint8)

    if len(id) != 16:
        raise ValueError("TensorId must be 16 bytes")

    return id
