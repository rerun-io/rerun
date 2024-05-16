from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint as rrb


def test_background_construction() -> None:
    rr.set_strict_mode(True)

    assert rrb.Background((1.0, 0.0, 0.0)) == rrb.Background(color=(1.0, 0.0, 0.0), kind=rrb.BackgroundKind.SolidColor)
    assert rrb.Background(rrb.BackgroundKind.GradientBright) == rrb.Background(
        color=None, kind=rrb.BackgroundKind.GradientBright
    )

    with pytest.raises(ValueError):
        rrb.Background(rrb.BackgroundKind.GradientBright, kind=rrb.BackgroundKind.GradientDark)
    with pytest.raises(ValueError):
        rrb.Background(rrb.BackgroundKind.GradientBright, color=(0.0, 1.0, 0.0))
    with pytest.raises(ValueError):
        rrb.Background((1.0, 0.0, 0.0), kind=rrb.BackgroundKind.GradientDark)
    with pytest.raises(ValueError):
        rrb.Background((1.0, 0.0, 0.0), color=(0.0, 1.0, 0.0))
