from __future__ import annotations

# TODO(grtlr): Promote to example

import os
import random
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

# TODO(grtlr): Clean up the exports
from rerun.blueprint.archetypes.force_collision_radius import ForceCollisionRadius
from rerun.blueprint.archetypes.force_link import ForceLink
from rerun.blueprint.archetypes.force_many_body import ForceManyBody
from rerun.components.show_labels import ShowLabels
from rerun.datatypes.rgba32 import Rgba32

README = """\
# Time-varying graph view

Please watch out for any twitching, jumping, or other wise unexpected changes to
the layout when navigating the timeline.

Please check the following:
* Scrub the timeline to see how the graph layout changes over time.
"""

color_scheme = [
    Rgba32([228, 26, 28, 255]),  # Red
    Rgba32([55, 126, 184, 255]),  # Blue
    Rgba32([77, 175, 74, 255]),  # Green
    Rgba32([152, 78, 163, 255]),  # Purple
    Rgba32([255, 127, 0, 255]),  # Orange
    Rgba32([255, 255, 51, 255]),  # Yellow
    Rgba32([166, 86, 40, 255]),  # Brown
    Rgba32([247, 129, 191, 255]),  # Pink
    Rgba32([153, 153, 153, 255]),  # Gray
]


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_graphs() -> None:
    nodes = ["root"]
    radii = [42]
    colors = [Rgba32([81, 81, 81, 255])]
    edges = []

    # We want reproducible results
    random.seed(42)

    # Randomly add nodes and edges to the graph
    for i in range(50):
        existing = random.choice(nodes)
        new_node = str(i)
        nodes.append(new_node)
        radii.append(random.randint(10, 50))
        colors.append(random.choice(color_scheme))
        edges.append((existing, new_node))

        rr.set_time_sequence("frame", i)
        rr.log(
            "node_link",
            rr.GraphNodes(nodes, labels=nodes, radii=radii, colors=colors),
            rr.GraphEdges(edges, graph_type=rr.GraphType.Directed),
        )
        rr.log(
            "bubble_chart",
            rr.GraphNodes(nodes, labels=nodes, radii=radii, colors=colors),
        )

    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Grid(
                rrb.GraphView(
                    origin="node_link",
                    name="Node-link diagram",
                    force_link=ForceLink(distance=60),
                    force_many_body=ForceManyBody(strength=-60),
                ),
                rrb.GraphView(
                    origin="bubble_chart",
                    name="Bubble chart",
                    force_link=ForceLink(enabled=False),
                    force_many_body=ForceManyBody(enabled=False),
                    force_collision_radius=ForceCollisionRadius(enabled=True),
                    defaults=[ShowLabels(False)],
                ),
                rrb.TextDocumentView(origin="readme", name="Instructions"),
            )
        )
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_graphs()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
