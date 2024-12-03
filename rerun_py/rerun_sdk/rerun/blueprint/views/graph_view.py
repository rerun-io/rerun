# DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/python/mod.rs
# Based on "crates/store/re_types/definitions/rerun/blueprint/views/graph.fbs".

from __future__ import annotations

from typing import Union

__all__ = ["GraphView"]


from ... import datatypes
from ..._baseclasses import AsComponents, ComponentBatchLike
from ...datatypes import EntityPathLike, Utf8Like
from .. import archetypes as blueprint_archetypes
from ..api import SpaceView, SpaceViewContentsLike


class GraphView(SpaceView):
    """
    **View**: A graph view to display time-variying, directed or undirected graph visualization.

    Example
    -------
    ### Use a blueprint to create a graph view.:
    ```python
    import rerun as rr
    import rerun.blueprint as rrb

    rr.init("rerun_example_graph_view", spawn=True)

    rr.log(
        "simple",
        rr.GraphNodes(
            node_ids=["a", "b", "c"], positions=[(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)], labels=["A", "B", "C"]
        ),
    )

    # Create a Spatial2D view to display the points.
    blueprint = rrb.Blueprint(
        rrb.GraphView(
            origin="/",
            name="Graph",
            # Note that this translates the viewbox.
            visual_bounds=rrb.VisualBounds2D(x_range=[-150, 150], y_range=[-50, 150]),
        ),
        collapse_panels=True,
    )

    rr.send_blueprint(blueprint)
    ```
    <center>
    <picture>
      <source media="(max-width: 480px)" srcset="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/480w.png">
      <source media="(max-width: 768px)" srcset="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/768w.png">
      <source media="(max-width: 1024px)" srcset="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/1024w.png">
      <source media="(max-width: 1200px)" srcset="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/1200w.png">
      <img src="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/full.png" width="640">
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
        visual_bounds: blueprint_archetypes.VisualBounds2D | None = None,
        force_position_x: blueprint_archetypes.ForcePositionX | None = None,
        force_position_y: blueprint_archetypes.ForcePositionY | None = None,
    ) -> None:
        """
        Construct a blueprint for a new GraphView view.

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
            List of default components or component batches to add to the space view. When an archetype
            in the view is missing a component included in this set, the value of default will be used
            instead of the normal fallback for the visualizer.
        overrides:
            Dictionary of overrides to apply to the space view. The key is the path to the entity where the override
            should be applied. The value is a list of component or component batches to apply to the entity.

            Important note: the path must be a fully qualified entity path starting at the root. The override paths
            do not yet support `$origin` relative paths or glob expressions.
            This will be addressed in <https://github.com/rerun-io/rerun/issues/6673>.
        visual_bounds:
            Everything within these bounds is guaranteed to be visible.

            Somethings outside of these bounds may also be visible due to letterboxing.
        force_position_x:
            Adds a force that pull nodes towards an `x` position.
        force_position_y:
            Adds a force that pull nodes towards an `y` position.

        """

        properties: dict[str, AsComponents] = {}
        if visual_bounds is not None:
            if not isinstance(visual_bounds, blueprint_archetypes.VisualBounds2D):
                visual_bounds = blueprint_archetypes.VisualBounds2D(visual_bounds)
            properties["VisualBounds2D"] = visual_bounds

        if force_position_x is not None:
            if not isinstance(force_position_x, blueprint_archetypes.ForcePositionX):
                force_position_x = blueprint_archetypes.ForcePositionX(force_position_x)
            properties["ForcePositionX"] = force_position_x

        if force_position_y is not None:
            if not isinstance(force_position_y, blueprint_archetypes.ForcePositionY):
                force_position_y = blueprint_archetypes.ForcePositionY(force_position_y)
            properties["ForcePositionY"] = force_position_y

        super().__init__(
            class_identifier="Graph",
            origin=origin,
            contents=contents,
            name=name,
            visible=visible,
            properties=properties,
            defaults=defaults,
            overrides=overrides,
        )
