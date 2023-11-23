import argparse
import pathlib

import rerun as rr


def download_minisplit(root_dir: pathlib.Path) -> None:
    """
    Download nuScenes minisplit.

    Adopted from https://colab.research.google.com/github/nutonomy/nuscenes-devkit/blob/master/python-sdk/tutorials/nuscenes_tutorial.ipynb
    """
    # TODO(leo) implement this
    pass


def ensure_scene_available(root_dir: pathlib.Path, scene_id: str) -> None:
    """
    Ensure that the specified scene is available.

    Downloads minisplit into root_dir if scene_id is part of it and root_dir is empty.

    Raises ValueError if scene is not available and cannot be downloaded.
    """
    MINISPLIT_IDS = []


def log_nuscenes() -> None:
    pass


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualizes the nuScenes dataset using the Rerun SDK.")
    parser.add_argument(
        "--root_dir",
        type=pathlib.Path,
        default="dataset",
        help="Root directory of nuScenes dataset",
    )
    parser.add_argument("--scene_id", type=str, help="Scene id to visualize")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_nuscenes")
    recording_path = ensure_scene_available(args.root_dir, args.scene_id)
    log_nuscenes(recording_path, args.include_highres)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
