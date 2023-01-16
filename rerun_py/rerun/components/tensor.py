from __future__ import annotations
from typing import Dict, Final, Iterable, Optional, Union, cast
import uuid

import numpy as np
import numpy.typing as npt
import pyarrow as pa
from rerun.components import REGISTERED_FIELDS, ComponentTypeFactory

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

DTYPE_MAP: Final[Dict[TensorDType, str]] = {
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
    np.float64: "f64",
}


def build_meaning_unknown() -> pa.UnionArray:
    type_ids = pa.array([0], type=pa.int8())
    value_offsets = pa.array([0], type=pa.int32())
    return pa.Array.from_buffers(
        type=TensorType.storage_type["meaning"].type,
        length=1,
        buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
        children=[
            pa.array([True], type=pa.bool_()),
            pa.array([], type=pa.bool_()),
        ],
    )


def build_dense_union(data_type: pa.DenseUnionType, discriminant: str, data: pa.Array) -> pa.UnionArray:
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
        )
    except ValueError as e:
        print(e)
        raise ValueError(e.args)


class TensorArray(pa.ExtensionArray):  # type: ignore[misc]
    def from_numpy(
        array: npt.NDArray[TensorDType],
        names: Optional[Iterable[Optional[str]]] = None,
    ) -> TensorArray:
        """Build a `TensorArray` from an numpy array."""

        tensor_id = pa.repeat(pa.scalar(uuid.uuid4().bytes, type=TensorType.storage_type["tensor_id"].type), 1)

        if names:
            # if len(names) != len(array):
            #    raise TypeError("`names` doesn't match tensor shape")

            shape = pa.array(
                [[{"name": x[0], "size": x[1]} for x in zip(names, array.shape)]],
                type=TensorType.storage_type["shape"].type,
            )
        else:
            shape = pa.array(
                [[{"name": None, "size": x} for x in array.shape]], type=TensorType.storage_type["shape"].type
            )

        # shape = pa.repeat(pa.scalar([{"name": None, "size": 4}], type=TensorType.storage_type["shape"].type), 1)

        data_storage = pa.array(array.flatten())
        data_inner = pa.ListArray.from_arrays(pa.array([0, len(data_storage)]), data_storage)

        data = build_dense_union(
            TensorType.storage_type["data"].type,
            discriminant=DTYPE_MAP[cast(TensorDType, array.dtype.type)],
            data=data_inner,
        )
        meaning = build_meaning_unknown()

        storage = pa.StructArray.from_arrays(
            [
                tensor_id,
                shape,
                data,
                meaning,
            ],
            fields=list(TensorType.storage_type),
        ).cast(TensorType.storage_type)
        # TODO(john) enable extension type wrapper
        # return cast(TensorArray, pa.ExtensionArray.from_storage(TensorType(), storage))
        return storage  # type: ignore[no-any-return]


TensorType = ComponentTypeFactory("TensorType", TensorArray, REGISTERED_FIELDS["rerun.tensor"])

pa.register_extension_type(TensorType())
