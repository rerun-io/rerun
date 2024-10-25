#!/usr/bin/env python3
"""Examples of logging graph data to Rerun."""

from __future__ import annotations

import rerun as rr


def main() -> None:
    rr.init("rerun_example_py_node_link", spawn=True)

    s = 3  # scaling factor for the positions

    # Potentially unbalanced and not sorted :nerd_face:.
    # :warning: The nodes have to be unique, which is why we use `5_0`...
    rr.log(
        "binary_tree",
        rr.GraphNodes(
            ["1", "7", "2", "6", "5_0", "11", "9_0", "9_1", "5_1"],
            labels=["1", "7", "2", "6", "5", "11", "9", "9", "5"],
            positions=[
                (0 * s, 0 * s),  # 1
                (-20 * s, 30 * s),  # 7
                (-30 * s, 60 * s),  # 2
                (-10 * s, 60 * s),  # 6
                (-20 * s, 90 * s),  # 5_0
                (0 * s, 90 * s),  # 11
                (20 * s, 30 * s),  # 9_0
                (30 * s, 60 * s),  # 9_1
                (20 * s, 90 * s),  # 5_1
            ],
        ),
    )
    rr.log(
        "binary_tree",
        rr.GraphEdges(
            [
                rr.components.GraphEdge(source="1", target="7"),
                rr.components.GraphEdge(source="7", target="2"),
                rr.components.GraphEdge(source="7", target="6"),
                rr.components.GraphEdge(source="6", target="5_0"),
                rr.components.GraphEdge(source="6", target="11"),
                rr.components.GraphEdge(source="1", target="9_0"),
                rr.components.GraphEdge(source="9_0", target="9_1"),
                rr.components.GraphEdge(source="9_1", target="5_1"),
            ],
            graph_type="directed",
        ),
    )


if __name__ == "__main__":
    main()
