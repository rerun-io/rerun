#!/usr/bin/env python3
"""Examples of logging graph data to Rerun."""

from __future__ import annotations

import rerun as rr

s = 3  # scaling factor for the positions

# Potentially unbalanced and not sorted binary tree. :nerd_face:.
# :warning: The nodes have to be unique, which is why we use `5_0`…

NodeId = str


class NodeInfo:
    def __init__(self, label: str, pos: tuple[float, float]) -> None:
        self.label = label
        self.pos = pos


all_nodes: dict[NodeId, NodeInfo] = {
    "1": NodeInfo(label="1", pos=(0 * s, 0 * s)),
    "7": NodeInfo(label="7", pos=(-20 * s, 30 * s)),
    "2": NodeInfo(label="2", pos=(-30 * s, 60 * s)),
    "6": NodeInfo(label="6", pos=(-10 * s, 60 * s)),
    "5_0": NodeInfo(label="5", pos=(-20 * s, 90 * s)),
    "11": NodeInfo(label="11", pos=(0 * s, 90 * s)),
    "9_0": NodeInfo(label="9", pos=(20 * s, 30 * s)),
    "9_1": NodeInfo(label="9", pos=(30 * s, 60 * s)),
    "5_1": NodeInfo(label="5", pos=(20 * s, 90 * s)),
}


class Level:
    def __init__(self, nodes: list[NodeId], edges: list[tuple[NodeId, NodeId]]):
        self.nodes = nodes
        self.edges = edges


levels: list[Level] = [
    Level(nodes=["1"], edges=[]),
    Level(nodes=["1", "7", "9_0"], edges=[("1", "7"), ("1", "9_0")]),
    Level(
        nodes=["1", "7", "9_0", "2", "6", "9_1"],
        edges=[("1", "7"), ("1", "9_0"), ("7", "2"), ("7", "6"), ("9_0", "9_1")],
    ),
    Level(
        nodes=["1", "7", "9_0", "2", "6", "9_1", "5_0", "11", "5_1"],
        edges=[
            ("1", "7"),
            ("1", "9_0"),
            ("7", "2"),
            ("7", "6"),
            ("9_0", "9_1"),
            ("6", "5_0"),
            ("6", "11"),
            ("9_1", "5_1"),
        ],
    ),
]


def main() -> None:
    rr.init("rerun_example_py_graph_binary_tree", spawn=True)

    t = 0
    for level in levels:
        if len(level.nodes) > 0:
            t = t + 1
            rr.set_time_seconds("stable_time", t)
            rr.log(
                "binary_tree",
                rr.GraphNodes(
                    level.nodes,
                    labels=list(map(lambda n: all_nodes[n].label, level.nodes)),
                    positions=list(map(lambda n: all_nodes[n].pos, level.nodes)),
                ),
            )

        if len(level.edges) > 0:
            t = t + 1
            rr.set_time_seconds("stable_time", t)
            rr.log("binary_tree", rr.GraphEdges(level.edges))


if __name__ == "__main__":
    main()
