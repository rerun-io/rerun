from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning

if TYPE_CHECKING:
    from collections.abc import Callable

rr.init("rerun_example_exceptions", spawn=False)
# Make sure strict mode isn't leaking in from another context
mem = rr.memory_recording()


def expect_warning(call: Callable[..., None], expected_warning: str) -> None:
    with pytest.warns(RerunWarning) as warnings:
        call()
        print("Logged warnings:")
        for warning in warnings:
            print(warning)
        assert len(warnings) == 1
        assert expected_warning in str(warnings[0])


def test_expected_warnings() -> None:
    # Always set strict mode to false in case it leaked from another test
    rr.set_strict_mode(False)

    expect_warning(
        lambda: rr.log("points", rr.Points3D([1, 2, 3, 4, 5])),
        "Expected either a flat array with a length multiple of 3 elements, or an array with shape (`num_elements`, 3). Shape of passed array was (5,).",
    )
    expect_warning(
        lambda: rr.log("points", rr.Points2D([1, 2, 3, 4, 5])),
        "Expected either a flat array with a length multiple of 2 elements, or an array with shape (`num_elements`, 2). Shape of passed array was (5,).",
    )
    expect_warning(
        lambda: rr.log("test_transform", rr.Transform3D(rotation=[1, 2, 3, 4, 5])),  # type: ignore[arg-type]
        "Rotation must be compatible with either RotationQuat or RotationAxisAngle",
    )
    expect_warning(
        # TODO(jleibs): This should ideally capture the field name as mat3x3 as above
        lambda: rr.log("test_transform", rr.Transform3D(mat3x3=[1, 2, 3, 4, 5])),
        "cannot reshape array of size 5 into shape (3,3))",
    )
    expect_warning(
        lambda: rr.log("test_transform", rr.datatypes.Vec3D([1, 0, 0])),  # type: ignore[arg-type]
        "Expected an object implementing rerun.AsComponents or an iterable of rerun.DescribedComponentBatch, but got",
    )
    expect_warning(
        lambda: rr.log("world/image", rr.Pinhole(focal_length=3)),
        "Must provide one of principal_point, resolution, or width/height)",
    )
