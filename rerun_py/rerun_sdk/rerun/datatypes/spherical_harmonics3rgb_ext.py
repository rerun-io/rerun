from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pyarrow as pa

from .._converters import to_np_float16

if TYPE_CHECKING:
    from . import SphericalHarmonics3RgbArrayLike

NUM_COEFFICIENTS = 15
NUM_CHANNELS = 3
NUM_VALUES = NUM_COEFFICIENTS * NUM_CHANNELS


class SphericalHarmonics3RgbExt:
    """Extension for [SphericalHarmonics3Rgb][rerun.datatypes.SphericalHarmonics3Rgb]."""

    @staticmethod
    def native_to_pa_array_override(data: SphericalHarmonics3RgbArrayLike, data_type: pa.DataType) -> pa.Array:
        # `SphericalHarmonics3Rgb` itself is array-like (it has `__array__`), which the
        # `npt.ArrayLike` in the alias doesn't capture.
        array = to_np_float16(cast("Any", data))

        # A coefficient-major `(15, 3)` and a channel-major `(3, 15)` array hold the same number
        # of values, and channel-major is exactly how the `f_rest_*` properties of a 3DGS PLY file
        # are laid out — so check the shape rather than silently transpose the caller's data.
        # Likewise, an unpadded lower-degree batch such as `(N, 8, 3)` must not be regrouped
        # across gaussians.
        shape = array.shape
        valid = (
            shape[-2:] == (NUM_COEFFICIENTS, NUM_CHANNELS)
            or shape[-1:] == (NUM_VALUES,)
            or (array.ndim == 1 and array.size % NUM_VALUES == 0)
        )
        if not valid:
            raise ValueError(
                f"Expected spherical harmonics coefficients of shape (…, {NUM_COEFFICIENTS}, {NUM_CHANNELS}) "
                f"or (…, {NUM_VALUES}), coefficient-major. Shape of the passed array was {shape}.",
            )

        array = np.ascontiguousarray(array.reshape(-1))
        array = pa.array(array, type=data_type.value_type.value_type)
        array = pa.FixedSizeListArray.from_arrays(array, type=data_type.value_type)
        return pa.FixedSizeListArray.from_arrays(array, type=data_type)
