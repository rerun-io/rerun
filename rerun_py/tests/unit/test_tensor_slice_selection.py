from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

from .common_arrays import none_empty_or_value


def test_tensor_slice_selection() -> None:
    widths = [
        None,
        2,
        rr.datatypes.TensorDimensionSelection(dimension=2, invert=False),
        rr.components.TensorWidthDimension(dimension=2, invert=False),
    ]
    heights = [
        None,
        3,
        rr.datatypes.TensorDimensionSelection(dimension=3, invert=False),
        rr.components.TensorHeightDimension(dimension=3, invert=False),
    ]
    indices_arrays = [
        [
            rr.components.TensorDimensionIndexSelection(dimension=1, index=3),
            rr.components.TensorDimensionIndexSelection(dimension=2, index=2),
            rr.components.TensorDimensionIndexSelection(dimension=3, index=1),
        ],
        None,
    ]
    slider_arrays = [
        None,
        [1, 2, 3],
        [
            rrb.components.TensorDimensionIndexSlider(1),
            rrb.components.TensorDimensionIndexSlider(2),
            rrb.components.TensorDimensionIndexSlider(3),
        ],
        np.array([1, 2, 3]),
    ]

    all_arrays = itertools.zip_longest(
        widths,
        heights,
        indices_arrays,
        slider_arrays,
    )

    for width, height, indices, slider in all_arrays:
        width = cast(Optional[rr.datatypes.TensorDimensionSelectionLike], width)
        height = cast(Optional[rr.datatypes.TensorDimensionSelectionLike], height)
        indices = cast(Optional[rr.datatypes.TensorDimensionIndexSelectionArrayLike], indices)
        slider = cast(Optional[rr.blueprint.datatypes.TensorDimensionIndexSliderArrayLike], slider)

        print(
            f"rr.TensorSliceSelection(\n"
            f"    width={width!r}\n"  #
            f"    height={height!r}\n"  #
            f"    indices={indices!r}\n"  #
            f"    slider={slider!r}\n"  #
            f")"
        )
        arch = rrb.TensorSliceSelection(
            width=width,
            height=height,
            indices=indices,
            slider=slider,
        )
        print(f"{arch}\n")

        assert arch.width == rr.components.TensorWidthDimensionBatch._optional(
            none_empty_or_value(width, rr.components.TensorWidthDimension(dimension=2, invert=False))
        )
        assert arch.height == rr.components.TensorHeightDimensionBatch._optional(
            none_empty_or_value(height, rr.components.TensorHeightDimension(dimension=3, invert=False))
        )
        assert arch.indices == rr.components.TensorDimensionIndexSelectionBatch._optional(
            none_empty_or_value(indices, indices_arrays[0])
        )
        assert arch.slider == rrb.components.TensorDimensionIndexSliderBatch._optional(
            none_empty_or_value(slider, [1, 2, 3])
        )
