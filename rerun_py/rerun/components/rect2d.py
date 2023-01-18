from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_FIELDS, ComponentTypeFactory, build_dense_union
from rerun.log.rects import RectFormat

__all__ = [
    "Rect2DArray",
    "Rect2DType",
]


class Rect2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy_and_format(array: npt.NDArray[np.float_], rect_format: RectFormat) -> Rect2DArray:
        """Build a `Rect2DArray` from an Nx4 numpy array."""
        rects = np.asarray(array, dtype="float32")
        # Inner is a FixedSizeList<4>
        values = pa.array(rects.flatten(), type=pa.float32())
        inner = pa.FixedSizeListArray.from_arrays(values=values, type=Rect2DType.storage_type[0].type)
        storage = build_dense_union(data_type=Rect2DType.storage_type, discriminant=rect_format.value, child=inner)
        storage.validate(full=True)
        # TODO(john) enable extension type wrapper
        # return cast(Rect2DArray, pa.ExtensionArray.from_storage(Rect2DType(), storage))
        return storage  # type: ignore[no-any-return]


Rect2DType = ComponentTypeFactory("Rect2DType", Rect2DArray, REGISTERED_FIELDS["rerun.rect2d"])

pa.register_extension_type(Rect2DType())
