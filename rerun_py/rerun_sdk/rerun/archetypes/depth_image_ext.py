from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .._validators import find_non_empty_dim_indices
from ..error_utils import _send_warning_or_raise, catch_and_log_exceptions

if TYPE_CHECKING:
    from ..components import TensorDataBatch
    from ..datatypes import TensorDataArrayLike


class DepthImageExt:
    """Extension for [DepthImage][rerun.archetypes.DepthImage]."""

    @staticmethod
    @catch_and_log_exceptions("DepthImage converter")
    def data__field_converter_override(data: TensorDataArrayLike) -> TensorDataBatch:
        from ..components import TensorDataBatch
        from ..datatypes import TensorDataType, TensorDimensionType

        tensor_data = TensorDataBatch(data)
        tensor_data_arrow = tensor_data.as_arrow_array()

        # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
        # once we coerce to a canonical non-arrow type.
        shape_dims = tensor_data_arrow[0].value["shape"].values.field(0).to_numpy()
        shape_names = tensor_data_arrow[0].value["shape"].values.field(1).to_numpy(zero_copy_only=False)

        non_empty_dims = find_non_empty_dim_indices(shape_dims)

        num_non_empty_dims = len(non_empty_dims)

        # TODO(#3239): What `recording` should we be passing here? How should we be getting it?
        if num_non_empty_dims != 2:
            _send_warning_or_raise(f"Expected depth image, got array of shape {shape_dims}", 1, recording=None)

        # IF no labels are set, add them
        # TODO(jleibs): Again, needing to do this at the arrow level is awful
        if all(label is None for label in shape_names):
            for ind, label in zip(non_empty_dims, ["height", "width"]):
                shape_names[ind] = label

            tensor_data_type = TensorDataType().storage_type
            shape_data_type = TensorDimensionType().storage_type

            shape_names = pa.array(
                shape_names, mask=np.array([n is None for n in shape_names]), type=shape_data_type.field("name").type
            )

            new_shape = pa.ListArray.from_arrays(
                offsets=[0, len(shape_dims)],
                values=pa.StructArray.from_arrays(
                    [
                        tensor_data_arrow[0].value["shape"].values.field(0),
                        shape_names,
                    ],
                    fields=[shape_data_type.field("size"), shape_data_type.field("name")],
                ),
            ).cast(tensor_data_type.field("shape").type)

            return TensorDataBatch(
                pa.StructArray.from_arrays(
                    [
                        new_shape,
                        tensor_data_arrow.storage.field(1),
                    ],
                    fields=[
                        tensor_data_type.field("shape"),
                        tensor_data_type.field("buffer"),
                    ],
                ).cast(tensor_data_arrow.storage.type)
            )

        # TODO(jleibs): Should we enforce specific names on images? Specifically, what if the existing names are wrong.
        return tensor_data
