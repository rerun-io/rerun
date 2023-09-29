from __future__ import annotations

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning

rr.init("exceptions", spawn=False)
# Make sure strict mode isn't leaking in from another context
mem = rr.memory_recording()


def test_expected_warnings() -> None:
    # Always set strict mode to false in case it leaked from another test
    rr.set_strict_mode(False)
    with pytest.warns(RerunWarning) as warnings:
        expected_warnings = [
            (
                rr.log("points", rr.Points3D([1, 2, 3, 4, 5])),
                "Expected either a flat array with a length multiple of 3 elements, or an array with shape (`num_elements`, 3). Shape of passed array was (5,).",
            ),
            (
                rr.log("points", rr.Points2D([1, 2, 3, 4, 5])),
                "Expected either a flat array with a length multiple of 2 elements, or an array with shape (`num_elements`, 2). Shape of passed array was (5,).",
            ),
        ]

        assert len(warnings) == len(expected_warnings)
        for warning, (_, expected) in zip(warnings, expected_warnings):
            assert expected in str(warning)
