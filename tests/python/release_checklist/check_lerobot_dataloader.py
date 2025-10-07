from __future__ import annotations

import os
from argparse import Namespace

import rerun as rr

README = """\
# LeRobot dataloader check

This will load a small LeRobot dataset -- simply make sure that it does.

The LeRobot dataset loader works by creating a new _recording_ for each episode in the dataset.
I.e., you should see exactly 3 recordings, corresponding to episode 0, 1 and 2.
"""


def run(args: Namespace) -> None:
    # NOTE: The LeRobot dataloader works by creating a new recording for each episode.
    # That means the `recording_id` needs to be set to "episode_0", otherwise the LeRobot dataloader
    # will create a new recording for episode 0, instead of merging it into the existing recording.
    # If you don't set it, you'll end up with 4 recordings, an empty one and the 3 episodes.
    rec = rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id="episode_0")

    dataset_path = os.path.dirname(__file__) + "/../../../tests/assets/lerobot/apple_storage"
    rec.log_file_from_path(dataset_path)

    # NOTE: This dataloader works by creating a new recording for each episode.
    # So that means we need to log the README to each recording.
    for i in range(3):
        rec = rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=f"episode_{i}")
        rec.set_time("frame_index", sequence=0)
        rec.log("/readme", rr.TextDocument(README), static=True)

    rec.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
