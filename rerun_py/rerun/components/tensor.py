from __future__ import annotations

import uuid
from typing import Final, Iterable, Optional, Union, cast

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_FIELDS, ComponentTypeFactory, build_dense_union

from rerun import bindings

__all__ = [
    "TensorArray",
    "TensorType",
    "TensorDType",
]

TensorDType = Union[
    np.uint8,
    np.uint16,
    np.uint32,
    np.uint64,
    np.int8,
    np.int16,
    np.int32,
    np.int64,
    np.float16,
    np.float32,
    np.float64,
]

# Map array dtypes to supported Tensor discriminant values
DTYPE_MAP: Final[dict[npt.DTypeLike, str]] = {
    np.uint8: "U8",
    np.uint16: "U16",
    np.uint32: "U32",
    np.uint64: "U64",
    np.int8: "I8",
    np.int16: "I16",
    np.int32: "I32",
    np.int64: "I64",
    np.float16: "F16",
    np.float32: "F32",
    np.float64: "F64",
}


class TensorArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(
        array: npt.NDArray[TensorDType],
        names: Iterable[str | None] | None = None,
        meaning: bindings.TensorDataMeaning = None,
        meter: Optional[float] = None,
    ) -> TensorArray:
        """Build a `TensorArray` from an numpy array."""
        # Build a random tensor_id
        tensor_id = pa.repeat(pa.scalar(uuid.uuid4().bytes, type=TensorType.storage_type["tensor_id"].type), 1)

        if not names:
            names = [None] * len(array.shape)
        shape_data = [[{"name": x[0], "size": x[1]} for x in zip(names, array.shape)]]
        shape = pa.array(shape_data, type=TensorType.storage_type["shape"].type)

        if array.dtype == np.uint8:
            data_inner = pa.array([array.flatten().tobytes()], type=pa.binary())
        else:
            data_storage = pa.array(array.flatten())
            data_inner = pa.ListArray.from_arrays(pa.array([0, len(data_storage)]), data_storage)

        data = build_dense_union(
            TensorType.storage_type["data"].type,
            discriminant=DTYPE_MAP[cast(TensorDType, array.dtype.type)],
            child=data_inner,
        )

        meaning = build_dense_union(
            TensorType.storage_type["meaning"].type,
            discriminant=("ClassId" if meaning == bindings.TensorDataMeaning.ClassId else "Unknown"),
            child=pa.array([True], type=pa.bool_()),
        )

        if meter is None:
            meter = pa.array([0.0], mask=[True], type=pa.float32())
        else:
            meter = pa.array([meter], mask=[False], type=pa.float32())

        storage = pa.StructArray.from_arrays(
            [
                tensor_id,
                shape,
                data,
                meaning,
                meter,
            ],
            fields=list(TensorType.storage_type),
        ).cast(TensorType.storage_type)
        storage.validate(full=True)
        # TODO(john) enable extension type wrapper
        # return cast(TensorArray, pa.ExtensionArray.from_storage(TensorType(), storage))
        return storage  # type: ignore[no-any-return]


TensorType = ComponentTypeFactory("TensorType", TensorArray, REGISTERED_FIELDS["rerun.tensor"])

pa.register_extension_type(TensorType())
