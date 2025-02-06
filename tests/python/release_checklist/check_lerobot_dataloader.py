from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# LeRobot dataloader check

This will load an entire LeRobot dataset -- simply make sure that it does 🙃

The LeRobot dataset loader works by creating a new _recording_ (⚠)️ for each episode in the dataset.
I.e., you should see a bunch of recordings below this readme (10, to be exact).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    # NOTE: This dataloader works by creating a new recording for each episode.
    # Those recordings all share the same application_id though, which means they also share
    # the same blueprint: we cannot log a readme, or all the the recordings would show an empty
    # readme.
    # log_readme()
    print(README)

    dataset_path = os.path.dirname(__file__) + "/../../../tests/assets/lerobot/apple_storage"
    rr.log_file_from_path(dataset_path)

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
