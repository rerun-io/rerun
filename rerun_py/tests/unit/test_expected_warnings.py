from __future__ import annotations

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning

rr.init("exceptions", spawn=False)
mem = rr.memory_recording()


def test_points_warnings() -> None:
    with pytest.warns(RerunWarning) as warnings:
        rr.log("points", rr.Points3D([1, 2, 3, 4, 5]))

        assert len(warnings) == 1
        assert (
            "Expected either a flat array with a length a of 3 elements, or an array with shape (`num_elements`, 3). Shape of passed array was (5,).)"
            in str(warnings[0].message)
        )
