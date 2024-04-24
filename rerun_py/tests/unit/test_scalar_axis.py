from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint as rrb


def test_scalar_axis() -> None:
    rr.set_strict_mode(True)

    assert rrb.ScalarAxis() == rrb.ScalarAxis(range=None, lock_range_during_zoom=None)
    assert rrb.ScalarAxis(range=(0.0, 1.0)) == rrb.ScalarAxis(range=(0.0, 1.0), lock_range_during_zoom=None)
