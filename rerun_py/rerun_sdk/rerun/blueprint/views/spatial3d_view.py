# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/views/spatial3d.fbs".

from __future__ import annotations

from typing import Sequence, Union

__all__ = ["Spatial3DView"]


from ... import datatypes
from ..._baseclasses import AsComponents, ComponentBatchLike
from ...datatypes import EntityPathLike, Utf8Like
from .. import archetypes as blueprint_archetypes, components as blueprint_components
from ..api import SpaceView, SpaceViewContentsLike


class Spatial3DView(SpaceView):
    """
    **View**: For viewing spatial 3D data.

    Example
    -------
    ### Use a blueprint to customize a Spatial3DView.:
    ```python
    import rerun as rr
    import rerun.blueprint as rrb
    from numpy.random import default_rng

    rr.init("rerun_example_spatial_3d", spawn=True)

    # Create some random points.
    rng = default_rng(12345)
    positions = rng.uniform(-5, 5, size=[50, 3])
    colors = rng.uniform(0, 255, size=[50, 3])
    radii = rng.uniform(0.1, 0.5, size=[50])

    rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))
    rr.log("box", rr.Boxes3D(half_sizes=[5, 5, 5], colors=0))

    # Create a Spatial3D view to display the points.
    blueprint = rrb.Blueprint(
        rrb.Spatial3DView(
            origin="/",
            name="3D Scene",
            # Set the background color to light blue.
            background=[100, 149, 237],
            # Configure the line grid.
            line_grid=rrb.archetypes.LineGrid3D(
                visible=True,  # The grid is enabled by default, but you can hide it with this property.
                spacing=0.1,  # Makes the grid more fine-grained.
                # By default, the plane is inferred from view coordinates setup, but you can set arbitrary planes.
                plane=rr.components.Plane3D.XY.with_distance(-5.0),
                stroke_width=2.0,  # Makes the grid lines twice as thick as usual.
                color=[255, 255, 255, 128],  # Colors the grid a half-transparent white.
            ),
        ),
        collapse_panels=True,
    )

    rr.send_blueprint(blueprint)
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/1200w.png">
      <img src="https://static.rerun.io/spatial3d/4816694fc4176cc284ff30d9c8f06c936a625ac9/full.png" width="640">
    </picture>
    </center>

    """

    def __init__(
        self,
        *,
        origin: EntityPathLike = "/",
        contents: SpaceViewContentsLike = "$origin/**",
        name: Utf8Like | None = None,
        visible: datatypes.BoolLike | None = None,
        defaults: list[Union[AsComponents, ComponentBatchLike]] = [],
        overrides: dict[EntityPathLike, list[ComponentBatchLike]] = {},
        background: blueprint_archetypes.Background
        | datatypes.Rgba32Like
        | blueprint_components.BackgroundKindLike
        | None = None,
        line_grid: blueprint_archetypes.LineGrid3D | None = None,
        time_ranges: blueprint_archetypes.VisibleTimeRanges
        | datatypes.VisibleTimeRangeLike
        | Sequence[datatypes.VisibleTimeRangeLike]
        | None = None,
    ) -> None:
        """
        Construct a blueprint for a new Spatial3DView view.

        Parameters
        ----------
        origin:
            The `EntityPath` to use as the origin of this view.
            All other entities will be transformed to be displayed relative to this origin.
        contents:
            The contents of the view specified as a query expression.
            This is either a single expression, or a list of multiple expressions.
            See [rerun.blueprint.archetypes.SpaceViewContents][].
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
        background:
            Configuration for the background of the view.
        line_grid:
            Configuration for the 3D line grid.
        time_ranges:
            Configures which range on each timeline is shown by this view (unless specified differently per entity).

            If not specified, the default is to show the latest state of each component.
            If a timeline is specified more than once, the first entry will be used.

        """

        properties: dict[str, AsComponents] = {}
        if background is not None:
            if not isinstance(background, blueprint_archetypes.Background):
                background = blueprint_archetypes.Background(background)
            properties["Background"] = background

        if line_grid is not None:
            if not isinstance(line_grid, blueprint_archetypes.LineGrid3D):
                line_grid = blueprint_archetypes.LineGrid3D(line_grid)
            properties["LineGrid3D"] = line_grid

        if time_ranges is not None:
            if not isinstance(time_ranges, blueprint_archetypes.VisibleTimeRanges):
                time_ranges = blueprint_archetypes.VisibleTimeRanges(time_ranges)
            properties["VisibleTimeRanges"] = time_ranges

        super().__init__(
            class_identifier="3D",
            origin=origin,
            contents=contents,
            name=name,
            visible=visible,
            properties=properties,
            defaults=defaults,
            overrides=overrides,
        )
