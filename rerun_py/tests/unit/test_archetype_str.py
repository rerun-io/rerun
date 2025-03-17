from __future__ import annotations

import pytest
import rerun as rr


@pytest.mark.parametrize(
    ["archetype", "expected"],
    [
        [
            rr.Transform3D(),
            (
                "rr.Transform3D(\n"
                "    translation=[],\n"
                "    rotation_axis_angle=[],\n"
                "    quaternion=[],\n"
                "    scale=[],\n"
                "    mat3x3=[],\n"
                "    relation=[],\n"
                "    axis_length=[]\n"
                ")"
            ),
        ],
        [
            rr.Transform3D(translation=[10, 10, 10]),
            (
                "rr.Transform3D(\n"
                "    translation=[[10.0, 10.0, 10.0]],\n"
                "    rotation_axis_angle=[],\n"
                "    quaternion=[],\n"
                "    scale=[],\n"
                "    mat3x3=[],\n"
                "    relation=[],\n"
                "    axis_length=[]\n"
                ")"
            ),
        ],
        [
            rr.Points2D(positions=[[0, 0], [1, 1], [2, 2]]),
            "rr.Points2D(\n    positions=[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]\n)",
        ],
        [
            rr.Points2D(positions=[0, 0, 1, 1, 2, 2], radii=[4, 5, 6]),
            "rr.Points2D(\n    positions=[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]],\n    radii=[4.0, 5.0, 6.0]\n)",
        ],
        [rr.Points2D.from_fields(), "rr.Points2D()"],
    ],
)
def test_archetype_str(archetype: rr._baseclasses.Archetype, expected: str) -> None:
    assert str(archetype) == expected
