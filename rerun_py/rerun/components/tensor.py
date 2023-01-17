from __future__ import annotations

import uuid
from typing import Final, Iterable, Union, cast

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_FIELDS, ComponentTypeFactory

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


def build_dense_union(data_type: pa.DenseUnionType, discriminant: str, data: pa.Array) -> pa.UnionArray:
    """
    Build a dense UnionArray given the `data_type`, a discriminant, and the data value array.

    If the discriminant string doesn't match any possible value, a `ValueError` is raised.
    """
    try:
        idx = [f.name for f in list(data_type)].index(discriminant)
        type_ids = pa.array([idx], type=pa.int8())
        value_offsets = pa.array([0], type=pa.int32())
        children = [pa.nulls(0, type=f.type) for f in list(data_type)]
        children[idx] = data.cast(data_type[idx].type)
        return pa.Array.from_buffers(
            type=data_type,
            length=1,
            buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
            children=children,
        ).cast(data_type)
    except ValueError as e:
        raise ValueError(e.args)


class TensorArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(
        array: npt.NDArray[TensorDType],
        names: Iterable[str | None] | None = None,
        meaning: bindings.TensorDataMeaning = None,
    ) -> TensorArray:
        """Build a `TensorArray` from an numpy array."""
        # Build a random tensor_id
        tensor_id = pa.repeat(pa.scalar(uuid.uuid4().bytes, type=TensorType.storage_type["tensor_id"].type), 1)

        if not names:
            names = [None] * len(array.shape)
        shape_data = [[{"name": x[0], "size": x[1]} for x in zip(names, array.shape)]]
        shape = pa.array(shape_data, type=TensorType.storage_type["shape"].type)

        if array.dtype is np.uint8:  # type: ignore[comparison-overlap]
            data_inner = pa.array(array.flatten(), type=pa.binary())
        else:
            data_storage = pa.array(array.flatten())
            data_inner = pa.ListArray.from_arrays(pa.array([0, len(data_storage)]), data_storage)

        data = build_dense_union(
            TensorType.storage_type["data"].type,
            discriminant=DTYPE_MAP[cast(TensorDType, array.dtype.type)],
            data=data_inner,
        )

        meaning = build_dense_union(
            TensorType.storage_type["meaning"].type,
            discriminant=("Unknown" if meaning == bindings.TensorDataMeaning.Unknown else "ClassId"),
            data=pa.array([True], type=pa.bool_()),
        )

        storage = pa.StructArray.from_arrays(
            [
                tensor_id,
                shape,
                data,
                meaning,
            ],
            fields=list(TensorType.storage_type),
        ).cast(TensorType.storage_type)
        storage.validate(full=True)
        # TODO(john) enable extension type wrapper
        # return cast(TensorArray, pa.ExtensionArray.from_storage(TensorType(), storage))
        return storage  # type: ignore[no-any-return]


TensorType = ComponentTypeFactory("TensorType", TensorArray, REGISTERED_FIELDS["rerun.tensor"])

pa.register_extension_type(TensorType())
