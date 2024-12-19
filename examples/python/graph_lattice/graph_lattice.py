#!/usr/bin/env python3
"""Examples of logging graph data to Rerun and performing a force-based layout."""

from __future__ import annotations

import argparse
import itertools

import rerun as rr

NUM_NODES = 10

DESCRIPTION = """
# Graph Lattice
This is a minimal example that logs a graph (node-link diagram) that represents a lattice.

In this example, the node positions—and therefore the graph layout—are computed by Rerun internally.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/graph_lattice).
""".strip()


def log_data() -> None:
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    coordinates = itertools.product(range(NUM_NODES), range(NUM_NODES))

    nodes, colors = zip(*[
        (
            str(i),
            rr.components.Color([round((x / (NUM_NODES - 1)) * 255), round((y / (NUM_NODES - 1)) * 255), 0]),
        )
        for i, (x, y) in enumerate(coordinates)
    ])

    rr.log(
        "/lattice",
        rr.GraphNodes(
            nodes,
            colors=colors,
            labels=[f"({x}, {y})" for x, y in itertools.product(range(NUM_NODES), range(NUM_NODES))],
        ),
        static=True,
    )

    edges = []
    for x, y in itertools.product(range(NUM_NODES), range(NUM_NODES)):
        if y > 0:
            source = (y - 1) * NUM_NODES + x
            target = y * NUM_NODES + x
            edges.append((str(source), str(target)))
        if x > 0:
            source = y * NUM_NODES + (x - 1)
            target = y * NUM_NODES + x
            edges.append((str(source), str(target)))

    rr.log("/lattice", rr.GraphEdges(edges, graph_type="directed"), static=True)


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs a graph lattice using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_graph_lattice")
    log_data()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
