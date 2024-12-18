from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Mono-entity views

This test checks that mono-entity views work as expected.

- Reset the blueprint to default
- Check each view: when titled `ERROR`, they should display an error, and when titled `OK`, they should display the tensor or text document correctly.

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_data() -> None:
    rr.log("tensor/one", rr.Tensor(np.random.rand(10, 10, 3, 5)))
    rr.log("tensor/two", rr.Tensor(np.random.rand(3, 5, 7, 5)))

    rr.log("txt/one", rr.TextDocument("Hello"))
    rr.log("txt/two", rr.TextDocument("World"))


def blueprint() -> rrb.BlueprintLike:
    return rrb.Grid(
        rrb.TextDocumentView(origin="readme"),
        rrb.TensorView(origin="/tensor", name="ERROR"),
        rrb.TensorView(origin="/tensor/one", name="OK"),
        rrb.TensorView(origin="/tensor/two", name="OK"),
        rrb.TextDocumentView(origin="/txt", name="ERROR"),
        rrb.TextDocumentView(origin="/txt/one", name="OK"),
        rrb.TextDocumentView(origin="/txt/two", name="OK"),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_data()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
