from __future__ import annotations

import itertools
from typing import Optional, cast

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

from .common_arrays import none_empty_or_value


def test_scalar_axis() -> None:
    rr.set_strict_mode(True)

    # All from 42.1337 to 1337.42, but expressed differently
    ranges = [
        (42.1337, 1337.42),
        [42.1337, 1337.42],
        np.array([42.1337, 1337.42]),
        rr.components.Range1D([42.1337, 1337.42]),
        None,
    ]
    lock_range_during_zooms = [
        True,
        False,
    ]

    all_arrays = itertools.zip_longest(
        ranges,
        lock_range_during_zooms,
    )

    for range, lock_range_during_zoom in all_arrays:
        range = cast(Optional[rr.datatypes.Range1DLike], range)
        lock_range_during_zoom = cast(Optional[rr.datatypes.Bool], lock_range_during_zoom)

        print(
            f"rr.ScalarAxis(\n"
            f"    range={range!r}\n"  #
            f"    lock_range_during_zoom={lock_range_during_zoom!r}\n"
            f")"
        )
        arch = rrb.ScalarAxis(
            range=range,
            lock_range_during_zoom=lock_range_during_zoom,
        )
        print(f"{arch}\n")

        assert arch.range == rr.components.Range1DBatch._optional(none_empty_or_value(range, [42.1337, 1337.42]))
        assert arch.lock_range_during_zoom == rrb.components.LockRangeDuringZoomBatch._optional(lock_range_during_zoom)
