from __future__ import annotations

import pytest
import rerun as rr
import rerun.blueprint as rrb


def test_rulers_construction() -> None:
    rr.set_strict_mode(True)

    assert rrb.Rulers(True) == rrb.Rulers(visible=True)
    assert rrb.Rulers(True, name_x="u", name_y="v") == rrb.Rulers(visible=True, name_x="u", name_y="v")

    with pytest.raises(TypeError):
        rrb.Rulers(True, "u")  # type: ignore[misc]  # ty: ignore[too-many-positional-arguments]


def test_spatial2d_view_rulers() -> None:
    rr.set_strict_mode(True)

    from_bool = rrb.Spatial2DView(rulers=True)
    from_archetype = rrb.Spatial2DView(rulers=rrb.Rulers(visible=True))
    assert from_bool.properties["Rulers"] == from_archetype.properties["Rulers"]
