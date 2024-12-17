from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Graph view (multi- and self-edges)

Please check that both graph views show:
* two self-edges for `A`, a single one for `B`.
* Additionally, there should be:
    * two edges from `A` to `B`.
    * one edge from `B` to `A`.
    * one edge connecting `B` to an implicit node `C`.
    * a self-edge for `C`.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_multi_and_self_graph() -> None:
    rr.log(
        "graph",
        rr.GraphNodes(["A", "B"], labels=["A", "B"]),
        rr.GraphEdges(
            [
                # self-edges
                ("A", "A"),
                ("B", "B"),
                # duplicated edges
                ("A", "B"),
                ("A", "B"),
                ("B", "A"),
                # duplicated self-edges
                ("A", "A"),
                # implicit edges
                ("B", "C"),
                ("C", "C"),
            ],
            graph_type=rr.GraphType.Directed,
        ),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_multi_and_self_graph()
    log_readme()

    rr.send_blueprint(
        rrb.Grid(
            rrb.GraphView(origin="graph", name="Multiple edges and self-edges"),
            rrb.GraphView(
                origin="graph",
                name="Multiple edges and self-edges (without labels)",
                defaults=[rr.components.ShowLabels(False)],
            ),
            rrb.TextDocumentView(origin="readme", name="Instructions"),
        ),
        make_active=True,
        make_default=True,
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
