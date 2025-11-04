from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
from rerun.archetypes import Scalars

if TYPE_CHECKING:
    from rerun.datatypes import Float64ArrayLike

CASES: list[tuple[Float64ArrayLike, Float64ArrayLike]] = [
    (
        [],
        [],
    ),
    (0.5, [[0.5]]),
    (
        [0.333],
        [[0.333]],
    ),
    (
        [0.111, 0.222, 0.333],
        [[0.111], [0.222], [0.333]],
    ),
    (
        [[0.111, 0.222], [0.333, 0.444]],
        [[0.111, 0.222], [0.333, 0.444]],
    ),
    (np.array([1.1, 2.2, 3.3]), [[1.1], [2.2], [3.3]]),
    (np.array([[1.1, 1.2], [2.1, 2.2]]), [[1.1, 1.2], [2.1, 2.2]]),
    ((0.1, 0.2, 0.3), [[0.1], [0.2], [0.3]]),
    (np.array([]), []),
    (np.array([[0.5]]), [[0.5]]),
    (np.ones((4321, 4)), np.ones((4321, 4)).tolist()),
]


def test_scalars_columns() -> None:
    for input, expected in CASES:
        data = [*Scalars.columns(scalars=input)]
        assert data[0].as_arrow_array().to_pylist() == expected
