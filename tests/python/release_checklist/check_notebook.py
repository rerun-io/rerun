from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Notebook

Make sure to check that Google Colab works properly with the latest release candidate.
To do that, go to https://colab.research.google.com/drive/1R9I7s4o6wydQC_zkybqaSRFTtlEaked_
change the version at the top to the latest alpha/rc and step through the notebook (running all at once
might cause some viewers to stay empty).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def run(args: Namespace) -> None:
    rr.script_setup(
        args,
        f"{os.path.basename(__file__)}",
        recording_id=uuid4(),
    )
    rr.send_blueprint(rrb.Grid(rrb.TextDocumentView(origin="readme")), make_active=True, make_default=True)

    log_readme()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
