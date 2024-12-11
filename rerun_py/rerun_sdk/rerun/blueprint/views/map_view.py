# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/views/map.fbs".

from __future__ import annotations

from typing import Union

__all__ = ["MapView"]


from ... import datatypes
from ..._baseclasses import AsComponents, ComponentBatchLike
from ...datatypes import EntityPathLike, Utf8Like
from .. import archetypes as blueprint_archetypes, components as blueprint_components
from ..api import View, ViewContentsLike


class MapView(View):
    """
    **View**: A 2D map view to display geospatial primitives.

    Example
    -------
    ### Use a blueprint to create a map view.:
    ```python
    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_map_view", spawn=True)

    rr.log("points", rr.GeoPoints(lat_lon=[[47.6344, 19.1397], [47.6334, 19.1399]], radii=rr.Radius.ui_points(20.0)))

    # Create a map view to display the chart.
    blueprint = rrb.Blueprint(
        rrb.MapView(
            origin="points",
            name="MapView",
            zoom=16.0,
            background=rrb.MapProvider.OpenStreetMap,
        ),
        collapse_panels=True,
    )

    rr.send_blueprint(blueprint)
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/1200w.png">
      <img src="https://static.rerun.io/map_view/9d0a5ba3a6e8d4693ba98e1b3cfcc15d166fd41d/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self,
        *,
        origin: EntityPathLike = "/",
        contents: ViewContentsLike = "$origin/**",
        name: Utf8Like | None = None,
        visible: datatypes.BoolLike | None = None,
        defaults: list[Union[AsComponents, ComponentBatchLike]] = [],
        overrides: dict[EntityPathLike, list[ComponentBatchLike]] = {},
        zoom: blueprint_archetypes.MapZoom | datatypes.Float64Like | None = None,
        background: blueprint_archetypes.MapBackground | blueprint_components.MapProviderLike | None = None,
    ) -> None:
        """
        Construct a blueprint for a new MapView view.

        Parameters
        ----------
        origin:
            The `EntityPath` to use as the origin of this view.
            All other entities will be transformed to be displayed relative to this origin.
        contents:
            The contents of the view specified as a query expression.
            This is either a single expression, or a list of multiple expressions.
            See [rerun.blueprint.archetypes.ViewContents][].
        name:
            The display name of the view.
        visible:
            Whether this view is visible.

            Defaults to true if not specified.
        defaults:
            List of default components or component batches to add to the view. When an archetype
            in the view is missing a component included in this set, the value of default will be used
            instead of the normal fallback for the visualizer.
        overrides:
            Dictionary of overrides to apply to the view. The key is the path to the entity where the override
            should be applied. The value is a list of component or component batches to apply to the entity.

            Important note: the path must be a fully qualified entity path starting at the root. The override paths
            do not yet support `$origin` relative paths or glob expressions.
            This will be addressed in <https://github.com/rerun-io/rerun/issues/6673>.
        zoom:
            Configures the zoom level of the map view.
        background:
            Configuration for the background map of the map view.

        """

        properties: dict[str, AsComponents] = {}
        if zoom is not None:
            if not isinstance(zoom, blueprint_archetypes.MapZoom):
                zoom = blueprint_archetypes.MapZoom(zoom)
            properties["MapZoom"] = zoom

        if background is not None:
            if not isinstance(background, blueprint_archetypes.MapBackground):
                background = blueprint_archetypes.MapBackground(background)
            properties["MapBackground"] = background

        super().__init__(
            class_identifier="Map",
            origin=origin,
            contents=contents,
            name=name,
            visible=visible,
            properties=properties,
            defaults=defaults,
            overrides=overrides,
        )
