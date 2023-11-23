import argparse
import pathlib
import numpy as np

from nuscenes import nuscenes

import rerun as rr


def download_minisplit(root_dir: pathlib.Path) -> None:
    """
    Download nuScenes minisplit.

    Adopted from https://colab.research.google.com/github/nutonomy/nuscenes-devkit/blob/master/python-sdk/tutorials/nuscenes_tutorial.ipynb
    """
    # TODO(leo) implement this
    pass


def ensure_scene_available(root_dir: pathlib.Path, dataset_version: str, scene_name: str) -> None:
    """
    Ensure that the specified scene is available.

    Downloads minisplit into root_dir if scene_name is part of it and root_dir is empty.

    Raises ValueError if scene is not available and cannot be downloaded.
    """
    nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)
    # TODO handle this
    # try:
    # except:
    #     if dataset_version == "v1.0-mini":
    #         # TODO handle download case
    #     nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)

    scene_names = [s["name"] for s in nusc.scene]
    if scene_name not in scene_names:
        raise ValueError(f"{scene_name=} not found in dataset")


def log_nuscenes(root_dir: pathlib.Path, dataset_version: str, scene_name: str) -> None:
    nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)

    scene = next(s for s in nusc.scene if s["name"] == scene_name)

    # TODO log sensor configuration

    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)

    current_sample = nusc.get("sample", scene["first_sample_token"])
    start_timestamp = current_sample["timestamp"]
    while True:
        # log data
        for data_name, data_token in current_sample["data"].items():
            while True:
                meta_data = nusc.get("sample_data", data_token)
                rr.set_time_seconds("timestamp", (meta_data["timestamp"] - start_timestamp) * 1e-6)

                ego_pose = nusc.get("ego_pose", meta_data["ego_pose_token"])

                rotation_xyzw = np.roll(ego_pose["rotation"], shift=-1)
                rr.log(
                    "world/ego_vehicle",
                    rr.Transform3D(
                        translation=ego_pose["translation"],
                        rotation=rr.Quaternion(xyzw=rotation_xyzw),
                        from_parent=False,
                    ),
                )

                data_file_path = root_dir / meta_data["filename"]

                if meta_data["sensor_modality"] == "lidar":
                    # log lidar points
                    print(meta_data["ego_pose_token"])
                    pointcloud = nuscenes.LidarPointCloud.from_file(str(data_file_path))
                    points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
                    rr.log(f"world/ego_vehicle/{data_name}", rr.Points3D(points))
                elif meta_data["sensor_modality"] == "radar":
                    pointcloud = nuscenes.RadarPointCloud.from_file(str(data_file_path))
                    points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
                    rr.log(f"world/ego_vehicle/{data_name}", rr.Points3D(points))
                elif meta_data["sensor_modality"] == "camera":
                    # TODO log images
                    pass

                data_token = meta_data["next"]
                if data_token == "" or nusc.get("sample_data", data_token)["is_key_frame"]:
                    break

        # TODO optional log annotations

        if current_sample["next"] == "":
            break

        current_sample = nusc.get("sample", current_sample["next"])


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualizes the nuScenes dataset using the Rerun SDK.")
    parser.add_argument(
        "--root_dir",
        type=pathlib.Path,
        default="dataset",
        help="Root directory of nuScenes dataset",
    )
    parser.add_argument(
        "--scene_name", type=str, default="scene-0061", help="Scene name to visualize (typically of form 'scene-xxxx')"
    )
    parser.add_argument("--dataset_version", type=str, default="v1.0-mini", help="Scene id to visualize")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_nuscenes")

    ensure_scene_available(args.root_dir, args.dataset_version, args.scene_name)
    log_nuscenes(args.root_dir, args.dataset_version, args.scene_name)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
