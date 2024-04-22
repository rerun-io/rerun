from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint as rrb


def test_background_3d_construction() -> None:
    rr.set_strict_mode(True)

    assert rrb.Background3D((1.0, 0.0, 0.0)) == rrb.Background3D(
        color=(1.0, 0.0, 0.0), kind=rrb.Background3DKind.SolidColor
    )
    assert rrb.Background3D(rrb.Background3DKind.GradientBright) == rrb.Background3D(
        color=None, kind=rrb.Background3DKind.GradientBright
    )

    with pytest.raises(ValueError):
        rrb.Background3D(rrb.Background3DKind.GradientBright, kind=rrb.Background3DKind.GradientDark)
    with pytest.raises(ValueError):
        rrb.Background3D(rrb.Background3DKind.GradientBright, color=(0.0, 1.0, 0.0))
    with pytest.raises(ValueError):
        rrb.Background3D((1.0, 0.0, 0.0), kind=rrb.Background3DKind.GradientDark)
    with pytest.raises(ValueError):
        rrb.Background3D((1.0, 0.0, 0.0), color=(0.0, 1.0, 0.0))
