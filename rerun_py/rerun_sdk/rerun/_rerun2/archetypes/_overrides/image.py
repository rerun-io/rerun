from __future__ import annotations

from typing import TYPE_CHECKING

from rerun.log.error_utils import _send_warning

from ...datatypes import TensorDataArray

if TYPE_CHECKING:
    from ...datatypes import TensorDataArrayLike


def image_data_converter(data: TensorDataArrayLike) -> TensorDataArray:
    tensor_data = TensorDataArray.from_similar(data)

    # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
    # once we coerce to a canonical non-arrow type.
    shape = tensor_data[0].value["shape"].values.field(0).to_numpy()
    non_empty_dims = [d for d in shape if d != 1]
    num_non_empty_dims = len(non_empty_dims)

    # TODO(jleibs) how do we send warnings form down inside these converters?
    if num_non_empty_dims < 2 or 3 < num_non_empty_dims:
        _send_warning(f"Expected image, got array of shape {shape}", 1, recording=None)

    if num_non_empty_dims == 3:
        depth = shape[-1]
        if depth not in (1, 3, 4):
            _send_warning(
                f"Expected image depth of 1 (gray), 3 (RGB) or 4 (RGBA). Instead got array of shape {shape}",
                1,
                recording=None,
            )

    # TODO(jleibs): The rust code labels the tensor dimensions as well. Would be nice to do something
    # similar here if they are unnamed.

    # TODO(jleibs): Should we enforce specific names on images? Specifically, what if the existing names are wrong.

    return tensor_data
