from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.components import REGISTERED_COMPONENT_NAMES, ComponentTypeFactory
from rerun.log import _normalize_matrix3

__all__ = [
    "PinholeArray",
    "PinholeType",
]


@dataclass
class Pinhole:
    """Camera perspective projection (a.k.a. intrinsics)."""

    # Row-major intrinsics matrix for projecting from camera space to image space.
    image_from_cam: npt.ArrayLike

    # Pixel resolution (usually integers) of child image space. Width and height.
    resolution: npt.ArrayLike | None


class PinholeArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_pinhole(pinhole: Pinhole) -> PinholeArray:
        """Build a `PinholeArray` from a single pinhole."""

        image_from_cam = _normalize_matrix3(pinhole.image_from_cam)
        resolution = None if pinhole.resolution is None else np.array(pinhole.resolution, dtype=np.float32).flatten()
        storage = pa.StructArray.from_arrays(
            [
                pa.FixedSizeListArray.from_arrays(image_from_cam, type=PinholeType.storage_type["image_from_cam"].type),
                pa.FixedSizeListArray.from_arrays(resolution, type=PinholeType.storage_type["resolution"].type),
            ],
            fields=list(PinholeType.storage_type),
        )

        # TODO(clement) enable extension type wrapper
        # return cast(PinholeArray, pa.ExtensionArray.from_storage(PinholeType(), storage))
        return storage  # type: ignore[no-any-return]


PinholeType = ComponentTypeFactory("PinholeType", PinholeArray, REGISTERED_COMPONENT_NAMES["rerun.pinhole"])

pa.register_extension_type(PinholeType())
