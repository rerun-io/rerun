from __future__ import annotations

from typing import TYPE_CHECKING, cast

import numpy as np
import pyarrow as pa

from rerun.log.error_utils import _send_warning

if TYPE_CHECKING:
    from ...components import TensorDataArray
    from ...datatypes import TensorDataArrayLike


def image_data_converter(data: TensorDataArrayLike) -> TensorDataArray:
    from ...components import TensorDataArray
    from ...datatypes import TensorDataType, TensorDimensionType

    tensor_data = TensorDataArray.from_similar(data)

    # TODO(jleibs): Doing this on raw arrow data is not great. Clean this up
    # once we coerce to a canonical non-arrow type.
    shape_dims = tensor_data[0].value["shape"].values.field(0).to_numpy()
    shape_names = tensor_data[0].value["shape"].values.field(1).to_numpy(zero_copy_only=False)

    non_empty_dims = find_non_empty_dim_indices(shape_dims)

    num_non_empty_dims = len(non_empty_dims)

    # TODO(jleibs): What `recording` should we be passing here? How should we be getting it?
    if num_non_empty_dims < 2 or 3 < num_non_empty_dims:
        _send_warning(f"Expected image, got array of shape {shape_dims}", 1, recording=None)

    if num_non_empty_dims == 3:
        depth = shape_dims[non_empty_dims[-1]]
        if depth not in (3, 4):
            _send_warning(
                f"Expected image 3 (RGB) or 4 (RGBA). Instead got array of shape {shape_dims}",
                1,
                recording=None,
            )

    # IF no labels are set, add them
    # TODO(jleibs): Again, needing to do this at the arrow level is awful
    if all(label is None for label in shape_names):
        for ind, label in zip(non_empty_dims, ["height", "width", "depth"]):
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
                    tensor_data[0].value["shape"].values.field(0),
                    shape_names,
                ],
                fields=[shape_data_type.field("size"), shape_data_type.field("name")],
            ),
        ).cast(tensor_data_type.field("shape").type)

        return cast(
            TensorDataArray,
            TensorDataArray._EXTENSION_TYPE().wrap_array(
                pa.StructArray.from_arrays(
                    [
                        tensor_data.storage.field(0),
                        new_shape,
                        tensor_data.storage.field(2),
                    ],
                    fields=[
                        tensor_data_type.field("id"),
                        tensor_data_type.field("shape"),
                        tensor_data_type.field("buffer"),
                    ],
                ).cast(tensor_data.storage.type)
            ),
        )

    # TODO(jleibs): Should we enforce specific names on images? Specifically, what if the existing names are wrong.

    return tensor_data


# This code follows closely from `image_ext.rs`
def find_non_empty_dim_indices(shape: list[int]) -> list[int]:
    """Returns the indices of an appropriate set of non-empty dimensions."""
    if len(shape) < 2:
        return list(range(len(shape)))

    indices = list(d[0] for d in filter(lambda d: d[1] != 1, enumerate(shape)))

    # 0 must be valid since shape isn't empty or we would have returned an Err above
    first_non_empty = next(iter(indices), 0)
    last_non_empty = next(reversed(indices), first_non_empty)

    # Note, these are inclusive ranges

    # First, empty inner dimensions are more likely to be intentional than empty outer dimensions.
    # Grow to a min-size of 2.
    # (1x1x3x1) -> 3x1 mono rather than 1x1x3 RGB
    while (last_non_empty - first_non_empty) < 1 and last_non_empty < (len(shape) - 1):
        print(f"{last_non_empty} {first_non_empty} {len(shape)}")
        last_non_empty += 1

    target = 1
    if shape[last_non_empty] in (3, 4):
        target = 2

    # Next, consider empty outer dimensions if we still need them.
    # Grow up to 3 if the inner dimension is already 3 or 4 (Color Images)
    # Otherwise, only grow up to 2.
    # (1x1x3) -> 1x1x3 rgb rather than 1x3 mono
    while (last_non_empty - first_non_empty) < target and first_non_empty > 0:
        first_non_empty -= 1

    return list(range(first_non_empty, last_non_empty + 1))
