from __future__ import annotations

from typing import TYPE_CHECKING, Any, Sequence

if TYPE_CHECKING:
    from ..datatypes import TensorBufferLike, TensorData, TensorDimensionLike
    from ..datatypes.tensor_data_ext import TensorLike


class TensorExt:
    def __init__(
        self: Any,
        *,
        data: TensorData | None = None,
        shape: Sequence[TensorDimensionLike] | None = None,
        buffer: TensorBufferLike | None = None,
        array: TensorLike | None = None,
        names: Sequence[str | None] | None = None,
    ):
        """
        Construct a `Tensor` archetype.

        The `Tensor` archetype internally contains a single component: `TensorData`.

        You can construct a `Tensor` from an existing `TensorData` using the `data` kwarg,
        or alternatively specify `array`, `shape` and `buffer` as in the `TensorData` constructor.

        Parameters
        ----------
        self:
            The TensorData object to construct.
        data: TensorData | None
            A TensorData object to initialize the tensor with.
        shape: Sequence[TensorDimensionLike] | None
            The shape of the tensor. If None, and an array is provided, the shape will be inferred
            from the shape of the array.
        buffer: TensorBufferLike | None
            The buffer of the tensor. If None, and an array is provided, the buffer will be generated
            from the array.
        array: Tensor | None
            A numpy array (or The array of the tensor. If None, the array will be inferred from the buffer.
        names: Sequence[str] | None
            The names of the tensor dimensions when generating the shape from an array.
        """
        from ..datatypes import TensorData

        if len([x for x in (data, array, buffer) if x is not None]) != 1:
            raise ValueError("Must specify exactly one of 'data', 'array', or 'buffer'.")

        if not isinstance(data, TensorData):
            data = TensorData(shape=shape, buffer=buffer, array=array, names=names)

        self.__attrs_init__(data=data)
