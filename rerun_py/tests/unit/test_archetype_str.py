from __future__ import annotations

from unittest.mock import patch

import pytest
import rerun as rr


@pytest.mark.parametrize(
    ["archetype", "expected"],
    [
        [
            rr.Transform3D().from_fields(clear_unset=True),
            (
                "rr.Transform3D(\n"
                "  translation=[],\n"
                "  rotation_axis_angle=[],\n"
                "  quaternion=[],\n"
                "  scale=[],\n"
                "  mat3x3=[],\n"
                "  relation=[],\n"
                "  child_frame=[],\n"
                "  parent_frame=[]\n"
                ")"
            ),
        ],
        [
            rr.Transform3D(translation=[10, 10, 10]),
            ("rr.Transform3D(\n  translation=[[10.0, 10.0, 10.0]]\n)"),
        ],
        [
            rr.Points2D(positions=[[0, 0], [1, 1], [2, 2]]),
            "rr.Points2D(\n  positions=[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]\n)",
        ],
        [
            rr.Points2D(positions=[0, 0, 1, 1, 2, 2], radii=[4, 5, 6]),
            "rr.Points2D(\n  positions=[[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]],\n  radii=[4.0, 5.0, 6.0]\n)",
        ],
        [rr.Points2D.from_fields(), "rr.Points2D()"],
        [
            rr.Points3D(
                [
                    11,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    2,
                    3,
                    3,
                ],
                radii=[1, 2, 3],
            ),
            """\
rr.Points3D(
  positions=[[11.0, 2.0, 3.0], [2.0, 3.0, 2.0], [3.0, 2.0, 3.0], [2.0, 3.0, 2.0],
    [3.0, 2.0, 3.0], [2.0, 3.0, 2.0], [3.0, 2.0, 3.0], [2.0, 3.0, 2.0],
    [3.0, 2.0, 3.0], [2.0, 3.0, 2.0], [3.0, 2.0, 3.0], [2.0, 3.0, 3.0]],
  radii=[1.0, 2.0, 3.0]
)""",
        ],
    ],
)
def test_archetype_str(archetype: rr._baseclasses.Archetype, expected: str) -> None:
    assert str(archetype) == expected


def test_archetype_str_normalization() -> None:
    """Test that archetype names are correct regardless of import path."""
    # `import rerun`
    assert rr.Points3D.archetype() == "rerun.archetypes.Points3D"
    assert rr.Points3D.archetype_short_name() == "Points3D"

    # `import rerun_sdk.rerun`
    with patch.object(rr.Points3D, "__module__", "rerun_sdk.rerun.archetypes.points3d"):
        assert rr.Points3D.archetype() == "rerun.archetypes.Points3D"
        assert rr.Points3D.archetype_short_name() == "Points3D"
