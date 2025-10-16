"""Use a blueprint to customize a graph view."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_graph_view", spawn=True)

rr.log(
    "simple",
    rr.GraphNodes(
        node_ids=["a", "b", "c"],
        positions=[(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)],
        labels=["A", "B", "C"],
    ),
)

# Create a Spatial2D view to display the points.
blueprint = rrb.Blueprint(
    rrb.GraphView(
        origin="/",
        name="Graph",
        # Note that this translates the viewbox.
        visual_bounds=rrb.VisualBounds2D(x_range=[-150, 150], y_range=[-50, 150]),
        background=rrb.archetypes.GraphBackground(color=[30, 10, 10]),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
