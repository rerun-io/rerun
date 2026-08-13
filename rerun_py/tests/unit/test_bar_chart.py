from __future__ import annotations

import numpy as np
import pytest
import rerun as rr


def test_bar_chart_shapes() -> None:
    """`BarChart` accepts only 1D data."""
    rr.set_strict_mode(True)

    # Single-element 1D array.
    rr.BarChart(np.array([1.0]))
    # Regular 1D array.
    rr.BarChart(np.array([1.0, 2.0, 3.0]))
    # Leading singleton dimension.
    rr.BarChart(np.array([[1.0, 2.0, 3.0]]))

    with pytest.raises(ValueError, match="Bar chart data should only be 1D"):
        rr.BarChart(np.array(1.0))

    with pytest.raises(ValueError, match="Bar chart data should only be 1D"):
        rr.BarChart(np.ones((2, 2)))
