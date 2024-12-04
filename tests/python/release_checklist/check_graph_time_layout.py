from __future__ import annotations

import os
import random
from argparse import Namespace

from uuid import uuid4
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Graph view

Please check the following:
* Run the graph view in an endless loop and see if how it looks (TM).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_graphs() -> None:
    nodes = ["root"]
    edges = []

    for i in range(50):
        existing = random.choice(nodes)
        new_node = str(i)
        nodes.append(new_node)
        edges.append((existing, new_node))

        rr.set_time_sequence("frame", i)
        rr.log("graph", rr.GraphNodes(nodes, labels=nodes), rr.GraphEdges(edges, graph_type=rr.GraphType.Directed))

    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Grid(
                rrb.GraphView(origin="graph", name="Graph"),
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
