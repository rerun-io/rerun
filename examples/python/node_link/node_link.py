#!/usr/bin/env python3
"""Examples of logging graph data to Rerun."""

from __future__ import annotations

import rerun as rr

s = 3  # scaling factor for the positions

nodes = {
    "1": {"label": "1", "pos": (0 * s, 0 * s)},
    "7": {"label": "7", "pos": (-20 * s, 30 * s)},
    "2": {"label": "2", "pos": (-30 * s, 60 * s)},
    "6": {"label": "6", "pos": (-10 * s, 60 * s)},
    "5_0": {"label": "5", "pos": (-20 * s, 90 * s)},
    "11": {"label": "11", "pos": (0 * s, 90 * s)},
    "9_0": {"label": "9", "pos": (20 * s, 30 * s)},
    "9_1": {"label": "9", "pos": (30 * s, 60 * s)},
    "5_1": {"label": "5", "pos": (20 * s, 90 * s)},
}

levels = [
    {"nodes": ["1"], "edges": []},
    {"nodes": ["1", "7", "9_0"], "edges": [("1", "7"), ("1", "9_0")]},
    {
        "nodes": ["1", "7", "9_0", "2", "6", "9_1"],
        "edges": [("1", "7"), ("1", "9_0"), ("7", "2"), ("7", "6"), ("9_0", "9_1")],
    },
    {
        "nodes": ["1", "7", "9_0", "2", "6", "9_1", "5_0", "11", "5_1"],
        "edges": [
            ("1", "7"),
            ("1", "9_0"),
            ("7", "2"),
            ("7", "6"),
            ("9_0", "9_1"),
            ("6", "5_0"),
            ("6", "11"),
            ("9_1", "5_1"),
        ],
    },
]


def to_edge(e: tuple[str, str]) -> rr.components.GraphEdge:
    return rr.components.GraphEdge(source=e[0], target=e[1])


def main() -> None:
    rr.init("rerun_example_py_node_link", spawn=True)

    # Potentially unbalanced and not sorted :nerd_face:.
    # :warning: The nodes have to be unique, which is why we use `5_0`…

    t = 0
    for level in levels:
        if len(level["nodes"]) > 0:
            t = t + 1
            rr.set_time_seconds("stable_time", t)
            rr.log(
                "binary_tree",
                rr.GraphNodes(
                    level["nodes"],
                    labels=list(map(lambda n: nodes[n]["label"], level["nodes"])),
                    positions=list(map(lambda n: nodes[n]["pos"], level["nodes"])),
                ),
            )

        if len(level["edges"]) > 0:
            t = t + 1
            rr.set_time_seconds("stable_time", t)
            rr.log(
                "binary_tree",
                rr.GraphEdges(list(map(to_edge, level["edges"]))),
            )


if __name__ == "__main__":
    main()
