from __future__ import annotations

import uuid
from typing import TYPE_CHECKING, Final

import numpy as np
import numpy.typing as npt
import pyarrow as pa

if TYPE_CHECKING:
    from .. import TensorBufferLike, TensorDataArrayLike, TensorDimension, TensorIdLike


# TODO(jleibs): Move this somewhere common
def _build_dense_union(data_type: pa.DenseUnionType, discriminant: str, child: pa.Array) -> pa.Array:
    """
    Build a dense UnionArray given the `data_type`, a discriminant, and the child value array.

    If the discriminant string doesn't match any possible value, a `ValueError` is raised.
    """
    try:
        idx = [f.name for f in list(data_type)].index(discriminant)
        type_ids = pa.array([idx] * len(child), type=pa.int8())
        value_offsets = pa.array(range(len(child)), type=pa.int32())

        children = [pa.nulls(0, type=f.type) for f in list(data_type)]
        try:
            children[idx] = child.cast(data_type[idx].type, safe=False)
        except pa.ArrowInvalid:
            # Since we're having issues with nullability in union types (see below),
            # the cast sometimes fails but can be skipped.
            children[idx] = child

        return pa.Array.from_buffers(
            type=data_type,
            length=len(child),
            buffers=[None, type_ids.buffers()[1], value_offsets.buffers()[1]],
            children=children,
        )

    except ValueError as e:
        raise ValueError(e.args)


def _build_tensorid(id: TensorIdLike) -> pa.Array:
    from .. import TensorId, TensorIdType

    if isinstance(id, uuid.UUID):
        array = np.asarray(list(id.bytes), dtype=np.uint8)
    elif isinstance(id, TensorId):
        array = id.uuid
    else:
        raise ValueError("Unsupported TensorId input")

    data_type = TensorIdType().storage_type

    return pa.FixedSizeListArray.from_arrays(array, type=data_type)


def _build_shape_array(dims: list[TensorDimension]) -> pa.Array:
    from .. import TensorDimensionType

    data_type = TensorDimensionType().storage_type

    array = np.asarray([d.size for d in dims], dtype=np.uint64).flatten()
    names = pa.array([d.name for d in dims], mask=[d is None for d in dims], type=data_type.field("name").type)

    return pa.ListArray.from_arrays(
        offsets=[0, len(array)],
        values=pa.StructArray.from_arrays(
            [
                array,
                names,
            ],
            fields=[data_type.field("size"), data_type.field("name")],
        ),
    )


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


def _build_buffer_array(buffer: TensorBufferLike) -> pa.Array:
    from .. import TensorBuffer, TensorBufferType

    data_type = TensorBufferType().storage_type

    if isinstance(buffer, TensorBuffer):
        buffer = buffer.inner

    buffer = buffer.flatten()

    data_inner = pa.ListArray.from_arrays(pa.array([0, len(buffer)]), buffer)

    return _build_dense_union(
        data_type,
        discriminant=DTYPE_MAP[buffer.dtype.type],
        child=data_inner,
    )


def tensordata_native_to_pa_array(data: TensorDataArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import TensorData, TensorDimension

    if isinstance(data, np.ndarray):
        tensor_id = _build_tensorid(uuid.uuid4())
        shape = [TensorDimension(d) for d in data.shape]
        shape = _build_shape_array(shape).cast(data_type.field("shape").type)
        buffer = _build_buffer_array(data)

    elif isinstance(data, TensorData):
        tensor_id = _build_tensorid(data.id)
        shape = _build_shape_array(data.shape).cast(data_type.field("shape").type)
        buffer = _build_buffer_array(data.buffer)

    else:
        raise ValueError("Unsupported TensorData source")

    return pa.StructArray.from_arrays(
        [
            tensor_id,
            shape,
            buffer,
        ],
        fields=[data_type.field("id"), data_type.field("shape"), data_type.field("buffer")],
    ).cast(data_type)
