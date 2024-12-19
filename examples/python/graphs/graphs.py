#!/usr/bin/env python3
"""Examples of logging graph data to Rerun and performing force-based layouts."""

from __future__ import annotations

import argparse
import itertools
import random

import numpy as np
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.archetypes.force_collision_radius import ForceCollisionRadius
from rerun.blueprint.archetypes.force_link import ForceLink
from rerun.blueprint.archetypes.force_many_body import ForceManyBody
from rerun.components.color import Color
from rerun.components.radius import Radius
from rerun.components.show_labels import ShowLabels

color_scheme = [
    Color([228, 26, 28]),  # Red
    Color([55, 126, 184]),  # Blue
    Color([77, 175, 74]),  # Green
    Color([152, 78, 163]),  # Purple
    Color([255, 127, 0]),  # Orange
    Color([255, 255, 51]),  # Yellow
    Color([166, 86, 40]),  # Brown
    Color([247, 129, 191]),  # Pink
    Color([153, 153, 153]),  # Gray
]

DESCRIPTION = """
# Graphs
This example shows various graph visualizations that you can create using Rerun.
In this example, the node positions—and therefore the graph layout—are computed by Rerun internally using a force-based layout algorithm.

You can modify how these graphs look by changing the parameters of the force-based layout algorithm in the selection panel.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/graphs).
""".strip()


# We want reproducible results
random.seed(42)


def log_lattice(num_nodes: int) -> None:
    coordinates = itertools.product(range(num_nodes), range(num_nodes))

    nodes, colors = zip(*[
        (
            str(i),
            rr.components.Color([round((x / (num_nodes - 1)) * 255), round((y / (num_nodes - 1)) * 255), 0]),
        )
        for i, (x, y) in enumerate(coordinates)
    ])

    rr.log(
        "lattice",
        rr.GraphNodes(
            nodes,
            colors=colors,
            labels=[f"({x}, {y})" for x, y in itertools.product(range(num_nodes), range(num_nodes))],
        ),
        static=True,
    )

    edges = []
    for x, y in itertools.product(range(num_nodes), range(num_nodes)):
        if y > 0:
            source = (y - 1) * num_nodes + x
            target = y * num_nodes + x
            edges.append((str(source), str(target)))
        if x > 0:
            source = y * num_nodes + (x - 1)
            target = y * num_nodes + x
            edges.append((str(source), str(target)))

    rr.log("lattice", rr.GraphEdges(edges, graph_type="directed"), static=True)


def log_trees() -> None:
    nodes = ["root"]
    radii = [42]
    colors = [Color([81, 81, 81])]
    edges = []

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


def log_markov_chain() -> None:
    transition_matrix = np.array([
        [0.8, 0.1, 0.1],  # Transitions from sunny
        [0.3, 0.4, 0.3],  # Transitions from rainy
        [0.2, 0.3, 0.5],  # Transitions from cloudy
    ])
    state_names = ["sunny", "rainy", "cloudy"]
    # For this example, we use hardcoded positions.
    positions = [[0, 0], [150, 150], [300, 0]]
    inactive_color = Color([153, 153, 153])  # Gray
    active_colors = [
        Color([255, 127, 0]),  # Orange
        Color([55, 126, 184]),  # Blue
        Color([152, 78, 163]),  # Purple
    ]

    edges = [
        (state_names[i], state_names[j])
        for i in range(len(state_names))
        for j in range(len(state_names))
        if transition_matrix[i][j] > 0
    ]
    edges.append(("start", "sunny"))

    # We start in state "sunny"
    state = "sunny"

    for i in range(50):
        current_state_index = state_names.index(state)
        next_state_index = np.random.choice(range(len(state_names)), p=transition_matrix[current_state_index])
        state = state_names[next_state_index]
        colors = [inactive_color] * len(state_names)
        colors[next_state_index] = active_colors[next_state_index]

        rr.set_time_sequence("frame", i)
        rr.log(
            "markov_chain",
            rr.GraphNodes(state_names, labels=state_names, colors=colors, positions=positions),
            rr.GraphEdges(edges, graph_type="directed"),
        )


def log_blueprint() -> None:
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
                rrb.GraphView(
                    origin="lattice",
                    name="Lattice",
                    force_link=ForceLink(distance=60),
                    force_many_body=ForceManyBody(strength=-60),
                    defaults=[ShowLabels(False), Radius(10)],
                ),
                rrb.Horizontal(
                    rrb.GraphView(
                        origin="markov_chain",
                        name="Markov Chain",
                        # We don't need any forces for this graph, because the nodes have fixed positions.
                    ),
                    rrb.TextDocumentView(origin="description", name="Description"),
                ),
            )
        )
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs various graphs using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_graphs")
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    log_trees()
    log_lattice(10)
    log_markov_chain()
    log_blueprint()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
