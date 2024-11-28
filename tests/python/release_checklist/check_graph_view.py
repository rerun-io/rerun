from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Graph view

Please check the following:
* All graphs have a proper layout.
* `graph` has directed edges, while `graph2` has undirected edges.
* `graph` and `graph2` are shown in two different viewers.
* There is a third viewer, `Both`, that shows both `graph` and `graph2` in the same viewer.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_graphs() -> None:
    DATA = [
        ("A", None),
        ("B", None),
        ("C", None),
        (None, ("A", "B")),
        (None, ("B", "C")),
        (None, ("A", "C")),
    ]

    nodes = []
    edges = []

    for i, (new_node, new_edge) in enumerate(DATA):
        if new_node is not None:
            nodes.append(new_node)
        if new_edge is not None:
            edges.append(new_edge)

        rr.set_time_sequence("frame", i)
        rr.log("graph", rr.GraphNodes(nodes, labels=nodes), rr.GraphEdges(edges, graph_type=rr.GraphType.Directed))
        rr.log("graph2", rr.GraphNodes(nodes, labels=nodes), rr.GraphEdges(edges, graph_type=rr.GraphType.Undirected))

    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Grid(
                rrb.GraphView(origin="graph", name="Graph 1"),
                rrb.GraphView(origin="graph2", name="Graph 2"),
                rrb.GraphView(name="Both", contents=["/graph", "/graph2"]),
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
