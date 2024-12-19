"""Log a simple undirected graph."""

import rerun as rr

rr.init("rerun_example_graph_undirected", spawn=True)

rr.log(
    "simple",
    rr.GraphNodes(
        node_ids=["a", "b", "c"], positions=[(0.0, 100.0), (-100.0, 0.0), (100.0, 0.0)], labels=["A", "B", "C"]
    ),
    rr.GraphEdges(
        edges=[("a", "b"), ("b", "c"), ("c", "a")],
        # Optional: graphs are undirected by default.
        graph_type="undirected",
    ),
)
