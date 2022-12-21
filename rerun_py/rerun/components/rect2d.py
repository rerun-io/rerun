from __future__ import annotations

from typing import ClassVar, Type, cast

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_FIELDS, ComponentTypeFactory
from rerun.log.rects import RectFormat

__all__ = [
    "Rect2DArray",
    "Rect2DType",
]


class Rect2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy_and_format(array: npt.NDArray[np.float_], rect_format: RectFormat) -> Rect2DArray:
        """Build a `Rect2DArray` from an Nx4 numpy array."""

        rects = np.asarray(array, dtype="float32")

        if rect_format == RectFormat.XYWH:
            storage = pa.StructArray.from_arrays(
                arrays=[pa.array(c, type=pa.float32()) for c in rects.T], fields=list(Rect2DType.storage_type)
            )
            # TODO(john) enable extension type wrapper
            # return cast(Rect2DArray, pa.ExtensionArray.from_storage(Rect2DType(), storage))
            return storage  # type: ignore[no-any-return]

        else:
            raise NotImplementedError("RectFormat not yet implemented")


Rect2DType = ComponentTypeFactory("Rect2DType", Rect2DArray, REGISTERED_FIELDS["rerun.rect2d"])

pa.register_extension_type(Rect2DType())
