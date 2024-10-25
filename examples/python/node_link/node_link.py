#!/usr/bin/env python3
"""Examples of logging graph data to Rerun."""

from __future__ import annotations

import rerun as rr


def main() -> None:
    rr.init("rerun_example_py_node_link", spawn=True)

    rr.log("binary_tree", rr.GraphNodes(["a", "b", "c"]))


if __name__ == "__main__":
    main()
