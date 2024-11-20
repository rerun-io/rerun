"""Log a simple directed graph."""

import rerun as rr

rr.init("rerun_example_graph_directed", spawn=True)

rr.log(
    "simple",
    rr.GraphNodes(
        node_ids=["a", "b", "c"], positions=[(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)], labels=["A", "B", "C"]
    ),
    rr.GraphEdges(edges=[("a", "b"), ("b", "c"), ("c", "a")], graph_type="directed"),
)
