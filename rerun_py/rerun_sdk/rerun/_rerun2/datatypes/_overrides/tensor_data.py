from __future__ import annotations

import collections
import uuid
from math import prod
from typing import TYPE_CHECKING, Any, Final, Protocol, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun.log.error_utils import _send_warning

if TYPE_CHECKING:
    from .. import TensorBufferLike, TensorData, TensorDataArrayLike, TensorDimension, TensorDimensionLike, TensorIdLike


################################################################################
# Torch-like array converters
################################################################################


class TorchTensorLike(Protocol):
    """Describes what is need from a Torch Tensor to be loggable to Rerun."""

    def numpy(self, force: bool) -> npt.NDArray[Any]:
        ...


Tensor = Union[npt.ArrayLike, TorchTensorLike]
"""Type helper for a tensor-like object that can be logged to Rerun."""


def _to_numpy(tensor: Tensor) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


################################################################################
# Init overrides
################################################################################


def tensordata_init(
    self: TensorData,
    *,
    id: TensorIdLike | None = None,
    shape: Sequence[TensorDimensionLike] | None = None,
    buffer: TensorBufferLike | None = None,
    array: Tensor | None = None,
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
        array = _to_numpy(array)

        # If a shape we provided, it must match the array
        if resolved_shape:
            shape_tuple = tuple(d.size for d in resolved_shape)
            if shape_tuple != array.shape:
                _send_warning(
                    (
                        f"Provided array ({array.shape}) does not match shape argument ({shape_tuple}). "
                        + "Ignoring shape argument."
                    ),
                    2,
                )
            resolved_shape = None

        if resolved_shape is None:
            if names:
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

    expected_buffer_size = prod(d.size for d in self.shape)

    if len(self.buffer.inner) != expected_buffer_size:
        raise ValueError(
            f"Shape and buffer size do not match. {len(self.buffer.inner)} {self.shape}->{expected_buffer_size}"
        )


################################################################################
# Arrow converters
################################################################################


def tensordata_native_to_pa_array(data: TensorDataArrayLike, data_type: pa.DataType) -> pa.Array:
    from .. import TensorData

    # If it's a sequence, grab the first one
    if isinstance(data, collections.abc.Sequence):
        if len(data) != 1:
            raise ValueError("Tensors do not support batches")
        data = data[0]

    # If it's not a TensorData, it should be an NDArray-like. coerce it into TensorData with the
    # constructor.
    if not isinstance(data, TensorData):
        array = _to_numpy(data)
        data = TensorData(array=array)

    # Now build the actual arrow fields
    tensor_id = _build_tensorid(data.id)
    shape = _build_shape_array(data.shape).cast(data_type.field("shape").type)
    buffer = _build_buffer_array(data.buffer)

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
