#!/usr/bin/env python3
"""Examples of logging graph data to Rerun and performing a force-based layout."""

from __future__ import annotations

import itertools

import rerun as rr

NUM_NODES = 10


def main() -> None:
    rr.init("rerun_example_py_graph_lattice", spawn=True)

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


if __name__ == "__main__":
    main()
