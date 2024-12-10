from __future__ import annotations

from typing import TYPE_CHECKING

from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from ..components import TensorDataBatch
    from ..datatypes import TensorDataArrayLike


class BarChartExt:
    """Extension for [BarChart][rerun.archetypes.BarChart]."""

    @staticmethod
    @catch_and_log_exceptions("BarChart converter")
    def values__field_converter_override(data: TensorDataArrayLike) -> TensorDataBatch:
        from ..components import TensorDataBatch

        tensor_data = TensorDataBatch(data)

        # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
        # once we coerce to a canonical non-arrow type.
        shape_dims = tensor_data.as_arrow_array()[0][0].values.to_numpy()

        if len([d for d in shape_dims if d != 1]) != 1:
            _send_warning_or_raise(
                f"Bar chart data should only be 1D. Got values with shape: {shape_dims}",
                2,
                recording=None,
            )

        return tensor_data
