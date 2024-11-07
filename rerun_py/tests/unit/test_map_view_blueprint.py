from __future__ import annotations

import rerun.blueprint as rrb


def test_map_view_blueprint() -> None:
    """Various ways to create a `MapView` blueprint."""

    bp1 = rrb.MapView(origin="point", name="MapView", zoom=16, background="openstreetmap")
    bp2 = rrb.MapView(origin="point", name="MapView", zoom=rrb.components.ZoomLevel(16), background="openstreetmap")
    bp3 = rrb.MapView(
        origin="point", name="MapView", zoom=rrb.archetypes.MapZoom(16), background=rrb.MapProvider.OpenStreetMap
    )
    bp4 = rrb.MapView(
        origin="point",
        name="MapView",
        zoom=rrb.archetypes.MapZoom(rrb.components.ZoomLevel(16)),
        background=rrb.archetypes.MapBackground(rrb.MapProvider.OpenStreetMap),
    )

    assert bp1 == bp2 == bp3 == bp4
