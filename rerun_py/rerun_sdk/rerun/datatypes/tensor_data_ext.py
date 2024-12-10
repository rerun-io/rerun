from __future__ import annotations

import collections
from math import prod
from typing import TYPE_CHECKING, Any, Final, Protocol, Sequence, Union

import numpy as np
import numpy.typing as npt
import pyarrow as pa

from rerun._validators import flat_np_uint64_array_from_array_like
from rerun.error_utils import _send_warning_or_raise

from .._unions import build_dense_union


class TorchTensorLike(Protocol):
    """Describes what is need from a Torch Tensor to be loggable to Rerun."""

    def numpy(self, force: bool) -> npt.NDArray[Any]: ...


if TYPE_CHECKING:
    from . import TensorBufferLike, TensorDataArrayLike, TensorDataLike

    TensorLike = Union[TensorDataLike, TorchTensorLike]
    """Type helper for a tensor-like object that can be logged to Rerun."""


def _to_numpy(tensor: TensorLike) -> npt.NDArray[Any]:
    # isinstance is 4x faster than catching AttributeError
    if isinstance(tensor, np.ndarray):
        return tensor

    try:
        # Make available to the cpu
        return tensor.numpy(force=True)  # type: ignore[union-attr]
    except AttributeError:
        return np.array(tensor, copy=False)


class TensorDataExt:
    """Extension for [TensorData][rerun.datatypes.TensorData]."""

    # TODO(jleibs): Should also provide custom converters for shape / buffer
    # assignment that prevent the user from putting the TensorData into an
    # inconsistent state.

    def __init__(
        self: Any,
        *,
        shape: Sequence[int] | None = None,
        buffer: TensorBufferLike | None = None,
        array: TensorLike | None = None,
        dim_names: Sequence[str] | None = None,
    ) -> None:
        """
        Construct a `TensorData` object.

        The `TensorData` object is internally represented by three fields: `shape` and `buffer`.

        This constructor provides additional arguments 'array', and 'dim_names'. When passing in a
        multi-dimensional array such as a `np.ndarray`, the `shape` and `buffer` fields will be
        populated automagically.

        Parameters
        ----------
        self:
            The TensorData object to construct.
        shape:
            The shape of the tensor. If None, and an array is provided, the shape will be inferred
            from the shape of the array.
        buffer:
            The buffer of the tensor. If None, and an array is provided, the buffer will be generated
            from the array.
        array:
            A numpy array (or The array of the tensor. If None, the array will be inferred from the buffer.
        dim_names:
            The names of the tensor dimensions.

        """
        if array is None and buffer is None:
            raise ValueError("Must provide one of 'array' or 'buffer'")
        if array is not None and buffer is not None:
            raise ValueError("Can only provide one of 'array' or 'buffer'")
        if buffer is not None and shape is None:
            raise ValueError("If 'buffer' is provided, 'shape' is also required")

        from . import TensorBuffer
        from .tensor_data import _tensor_data__buffer__special_field_converter_override

        if shape is not None:
            resolved_shape = list(shape)
        else:
            resolved_shape = None

        # Figure out the shape
        if array is not None:
            array = _to_numpy(array)

            # If a shape we provided, it must match the array
            if resolved_shape:
                shape_tuple = tuple(d for d in resolved_shape)
                if shape_tuple != array.shape:
                    _send_warning_or_raise(
                        (
                            f"Provided array ({array.shape}) does not match shape argument ({shape_tuple}). "
                            + "Ignoring shape argument."
                        ),
                        2,
                    )
                resolved_shape = None

            if resolved_shape is None:
                resolved_shape = [size for size in array.shape]

        if resolved_shape is not None:
            self.shape: npt.NDArray[np.uint64] = resolved_shape
        else:
            # This shouldn't be possible but typing can't figure it out
            raise ValueError("No shape provided.")

        if buffer is not None:
            self.buffer = _tensor_data__buffer__special_field_converter_override(buffer)
        elif array is not None:
            self.buffer = TensorBuffer(array.flatten())

        self.names: list[str] | None = None
        if dim_names:
            if len(self.shape) == len(dim_names):
                self.names = dim_names
            else:
                _send_warning_or_raise(
                    (
                        f"len(shape) = {len(self.shape)} != "
                        + f"len(dim_names) = {len(dim_names)}. Ignoring tensor dimension names."
                    ),
                    2,
                )

        expected_buffer_size = prod(d for d in self.shape)
        if len(self.buffer.inner) != expected_buffer_size:
            raise ValueError(
                f"Shape and buffer size do not match. {len(self.buffer.inner)} {self.shape}->{expected_buffer_size}"
            )

    ################################################################################
    # Arrow converters
    ################################################################################

    @staticmethod
    def native_to_pa_array_override(data: TensorDataArrayLike, data_type: pa.DataType) -> pa.Array:
        from . import TensorData

        # If it's a sequence of a single TensorData, grab the first one
        if isinstance(data, collections.abc.Sequence):
            if len(data) > 0:
                if isinstance(data[0], TensorData):
                    if len(data) > 1:
                        raise ValueError("Tensors do not support batches")
                    data = data[0]

        # If it's not a TensorData, it should be an NDArray-like. coerce it into TensorData with the
        # constructor.
        if not isinstance(data, TensorData):
            array = _to_numpy(data)  # type: ignore[arg-type]
            data = TensorData(array=array)

        # Now build the actual arrow fields
        shape = pa.array([flat_np_uint64_array_from_array_like(data.shape, 1)], type=data_type.field("shape").type)
        buffer = _build_buffer_array(data.buffer)

        if data.names is None:
            names = pa.array([None], type=data_type.field("names").type)
        else:
            names = pa.array([data.names], type=data_type.field("names").type)

        return pa.StructArray.from_arrays(
            [
                shape,
                names,
                buffer,
            ],
            fields=data_type.fields,
        ).cast(data_type)

    def numpy(self: Any, force: bool) -> npt.NDArray[Any]:
        """Convert the TensorData back to a numpy array."""
        dims = [d for d in self.shape]
        return self.buffer.inner.reshape(dims)  # type: ignore[no-any-return]


################################################################################
# Internal construction helpers
################################################################################


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
    from . import TensorBuffer, TensorBufferBatch

    data_type = TensorBufferBatch._ARROW_DATATYPE

    if isinstance(buffer, TensorBuffer):
        buffer = buffer.inner

    buffer = buffer.flatten()

    data_inner = pa.ListArray.from_arrays(pa.array([0, len(buffer)]), buffer)

    assert buffer.dtype.type in DTYPE_MAP, f"Failed to find {buffer.dtype.type} in f{DTYPE_MAP}"
    discriminant = DTYPE_MAP[buffer.dtype.type]

    return build_dense_union(
        data_type,
        discriminant=discriminant,
        child=data_inner,
    )
