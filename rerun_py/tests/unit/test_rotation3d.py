from __future__ import annotations

from rerun.experimental import dt as rrd

from .common_arrays import (
    expected_rotations,
    rotations_arrays,
)


def test_rotation3d() -> None:
    for rotations in rotations_arrays:
        print(f"rrd.Rotation3DArray.from_similar({rotations})")
        datatype = rrd.Rotation3DArray.from_similar(rotations)
        print(f"{datatype}\n")
        expected = expected_rotations(rotations, rrd.Rotation3DArray)
        print(f"{expected}\n")

        assert datatype == expected_rotations(rotations, rrd.Rotation3DArray)


if __name__ == "__main__":
    test_rotation3d()
