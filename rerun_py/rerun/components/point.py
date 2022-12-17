from __future__ import annotations

from typing import cast
import numpy.typing as npt
import numpy as np
import pyarrow as pa

from rerun.components import ComponentTypeFactory, REGISTERED_FIELDS


class Point2DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float_]) -> Point2DArray:
        """Build a `Point2DArray` from an Nx2 numpy array."""

        # TODO(john) assert shape

        points = np.asarray(array, dtype="float32")
        storage = pa.StructArray.from_arrays(
            arrays=[pa.array(c, type=pa.float32()) for c in points.T],
            fields=list(Point2DType.storage_type),
        )
        return cast(Point2DArray, pa.ExtensionArray.from_storage(Point2DType(), storage))


Point2DType = ComponentTypeFactory("Point2DType", Point2DArray, REGISTERED_FIELDS["rerun.point2d"])

pa.register_extension_type(Point2DType())


class Point3DArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(array: npt.NDArray[np.float_]) -> Point3DArray:
        """Build a `Point3DArray` from an Nx3 numpy array."""

        # TODO(john) assert shape

        points = np.asarray(array, dtype="float32")
        storage = pa.StructArray.from_arrays(
            arrays=[pa.array(c, type=pa.float32()) for c in points.T],
            fields=list(Point3DType.storage_type),
        )
        return cast(Point3DArray, pa.ExtensionArray.from_storage(Point3DType(), storage))


Point3DType = ComponentTypeFactory("Point3DType", Point3DArray, REGISTERED_FIELDS["rerun.point3d"])

pa.register_extension_type(Point3DType())
