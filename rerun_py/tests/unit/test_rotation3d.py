from __future__ import annotations

from rerun.experimental import dt as rrd

from .common_arrays import (
    expected_rotations,
    rotations_arrays,
)


def test_rotation3d() -> None:
    for rotations in rotations_arrays:
        print(f"rrd.Rotation3DBatch({rotations})")
        datatype = rrd.Rotation3DBatch(rotations)
        print(f"{datatype}\n")

        assert datatype == expected_rotations(rotations, rrd.Rotation3DBatch)


if __name__ == "__main__":
    test_rotation3d()
