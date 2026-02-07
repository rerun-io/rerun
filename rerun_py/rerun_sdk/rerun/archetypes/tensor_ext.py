from __future__ import annotations

from typing import TYPE_CHECKING, Any

from ..error_utils import catch_and_log_exceptions

if TYPE_CHECKING:
    from collections.abc import Sequence

    from rerun.datatypes.range1d import Range1DLike

    from ..datatypes import TensorDataLike
    from ..datatypes.tensor_data_ext import TensorLike


class TensorExt:
    """Extension for [Tensor][rerun.archetypes.Tensor]."""

    def __init__(
        self: Any,
        data: TensorDataLike | TensorLike | None = None,
        *,
        dim_names: Sequence[str] | None = None,
        value_range: Range1DLike | None = None,
    ) -> None:
        """
        Construct a `Tensor` archetype.

        The `Tensor` archetype internally contains a single component: `TensorData`.

        See the `TensorData` constructor for more advanced options to interpret buffers
        as `TensorData` of varying shapes.

        For simple cases, you can pass array objects and optionally specify the names of
        the dimensions. The shape of the `TensorData` will be inferred from the array.

        Parameters
        ----------
        self:
            The TensorData object to construct.
        data:
            A TensorData object, or type that can be converted to a numpy array.
        dim_names:
            The names of the tensor dimensions when generating the shape from an array.
        value_range:
            The range of values to use for colormapping.

            If not specified, the range will be estimated from the data.

        """
        from ..datatypes import TensorData

        with catch_and_log_exceptions(context=self.__class__.__name__):
            if not isinstance(data, TensorData):
                data = TensorData(array=data, dim_names=dim_names)
            elif dim_names is not None:
                data = TensorData(buffer=data.buffer, dim_names=dim_names)

            self.__attrs_init__(data=data, value_range=value_range)
            return

        self.__attrs_clear__()
