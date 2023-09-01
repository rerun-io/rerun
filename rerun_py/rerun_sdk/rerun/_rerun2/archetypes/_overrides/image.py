from __future__ import annotations

from typing import TYPE_CHECKING

from ...datatypes import TensorDataArray

if TYPE_CHECKING:
    from ...datatypes import TensorDataArrayLike


def image_data_converter(data: TensorDataArrayLike) -> TensorDataArray:
    tensor_data = TensorDataArray.from_similar(data)

    # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
    # once we coerce to a canonical non-arrow type.
    dimensions = tensor_data[0].value["shape"].values.field(0).to_numpy()

    return tensor_data
