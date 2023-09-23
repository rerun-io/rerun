from __future__ import annotations

from typing import TYPE_CHECKING

from rerun.error_utils import _send_warning

if TYPE_CHECKING:
    from ..components import TensorDataArray
    from ..datatypes import TensorDataArrayLike


class BarChartExt:
    @staticmethod
    def values__field_converter_override(data: TensorDataArrayLike) -> TensorDataArray:
        from ..components import TensorDataArray

        tensor_data = TensorDataArray.from_similar(data)

        # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
        # once we coerce to a canonical non-arrow type.
        shape_dims = tensor_data[0].value["shape"].values.field(0).to_numpy()

        if len(shape_dims) != 1:
            _send_warning(
                f"Bar chart data should only be 1D. Got values with shape: {shape_dims}",
                2,
                recording=None,
            )

        return tensor_data
