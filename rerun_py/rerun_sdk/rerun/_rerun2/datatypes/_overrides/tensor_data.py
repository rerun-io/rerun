from __future__ import annotations

import uuid
from typing import TYPE_CHECKING, Final, Sequence

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.log.error_utils import _send_warning

if TYPE_CHECKING:
    from .. import TensorBufferLike, TensorData, TensorDataArrayLike, TensorDimension, TensorDimensionLike, TensorIdLike

################################################################################
# Init overrides
################################################################################


def tensordata_init(
    self: TensorData,
    *,
    id: TensorIdLike | None = None,
    shape: Sequence[TensorDimensionLike] | None = None,
    buffer: TensorBufferLike | None = None,
    array: npt.NDArray[np.float32]
    | npt.NDArray[np.float64]
    | npt.NDArray[np.int16]
    | npt.NDArray[np.int32]
    | npt.NDArray[np.int64]
    | npt.NDArray[np.int8]
    | npt.NDArray[np.uint16]
    | npt.NDArray[np.uint32]
    | npt.NDArray[np.uint64]
    | npt.NDArray[np.uint8]
    | None = None,
    names: Sequence[str] | None = None,
) -> None:
    if array is None and buffer is None:
        raise ValueError("Must provide one of 'array' or 'buffer'")
    if array is not None and buffer is not None:
        raise ValueError("Can only provide one of 'array' or 'buffer'")
    if buffer is not None and shape is None:
        raise ValueError("If 'buffer' is provided, 'shape' is also required")
    if shape is not None and names is not None:
        raise ValueError("Can only provide one of 'shape' or 'names'")

    from .. import TensorBuffer, TensorDimension
    from ..tensor_data import _tensordata_buffer_converter, _tensordata_id_converter

    # Assign an id if one wasn't provided
    if id:
        self.id = _tensordata_id_converter(id)
    else:
        self.id = _tensordata_id_converter(uuid.uuid4())

    if shape:
        resolved_shape = list(shape)
    else:
        resolved_shape = None

    # Figure out the shape
    if array is not None:
        # If a shape we provided, it must match the array
        if resolved_shape:
            shape_tuple = tuple(d.size for d in resolved_shape)
            if shape_tuple != array.shape:
                raise ValueError(f"Provided array ({array.shape}) does not match shape argument ({shape_tuple}).")
        elif names:
            if len(array.shape) != len(names):
                _send_warning(
                    (
                        f"len(array.shape) = {len(array.shape)} != "
                        + f"len(names) = {len(names)}. Dropping tensor dimension names."
                    ),
                    2,
                )
            resolved_shape = [TensorDimension(size, name) for size, name in zip(array.shape, names)]
        else:
            resolved_shape = [TensorDimension(size) for size in array.shape]

    if resolved_shape is not None:
        self.shape = resolved_shape
    else:
        # This shouldn't be possible but typing can't figure it out
        raise ValueError("No shape provided.")

    if buffer is not None:
        self.buffer = _tensordata_buffer_converter(buffer)
    elif array is not None:
        self.buffer = TensorBuffer(array.flatten())


################################################################################
# Arrow converters
################################################################################


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


################################################################################
# Internal construction helpers
################################################################################


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
