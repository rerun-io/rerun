#!/usr/bin/env python3
from __future__ import annotations

import argparse
import numbers
import pathlib
from typing import Any

import matplotlib
import numpy as np
import rerun as rr
from download_dataset import MINISPLIT_SCENES, download_minisplit
from nuscenes import nuscenes

cmap = matplotlib.colormaps["turbo_r"]
norm = matplotlib.colors.Normalize(
    vmin=3.0,
    vmax=75.0,
)


def ensure_scene_available(root_dir: pathlib.Path, dataset_version: str, scene_name: str) -> None:
    """
    Ensure that the specified scene is available.

    Downloads minisplit into root_dir if scene_name is part of it and root_dir is empty.

    Raises ValueError if scene is not available and cannot be downloaded.
    """
    try:
        nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)
    except AssertionError:  # dataset initialization failed
        if dataset_version == "v1.0-mini" and scene_name in MINISPLIT_SCENES:
            download_minisplit(root_dir)
            nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)
        else:
            print(f"Could not find dataset at {root_dir} and could not automatically download specified scene.")
            exit()

    scene_names = [s["name"] for s in nusc.scene]
    if scene_name not in scene_names:
        raise ValueError(f"{scene_name=} not found in dataset")


def log_nuscenes(root_dir: pathlib.Path, dataset_version: str, scene_name: str) -> None:
    nusc = nuscenes.NuScenes(version=dataset_version, dataroot=root_dir, verbose=True)

    scene = next(s for s in nusc.scene if s["name"] == scene_name)

    # each sensor only has to be logged once, maintain set of already logged sensors
    logged_sensor_tokens: set[str] = set()

    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)

    current_sample = nusc.get("sample", scene["first_sample_token"])
    start_timestamp = current_sample["timestamp"]
    while True:
        log_nuscenes_sample(current_sample, nusc, logged_sensor_tokens, start_timestamp)

        if current_sample["next"] == "":
            break
        current_sample = nusc.get("sample", current_sample["next"])


def log_nuscenes_sample(
    sample: dict[str, Any],
    nusc: nuscenes.NuScenes,
    logged_sensor_tokens: set[str],
    start_timestamp: numbers.Number,
) -> None:
    # each sample is a keyframe with annotations
    for sensor_name, sample_data_token in sample["data"].items():
        # TODO optional log annotations
        while True:
            sample_data = nusc.get("sample_data", sample_data_token)
            log_nuscenes_sample_data(sample_data, nusc, logged_sensor_tokens, start_timestamp)

            sample_data_token = sample_data["next"]
            if sample_data_token == "" or nusc.get("sample_data", sample_data_token)["is_key_frame"]:
                break


def log_nuscenes_sample_data(
    sample_data: dict[str, Any],
    nusc: nuscenes.NuScenes,
    logged_sensor_tokens: set[str],
    start_timestamp: numbers.Number,
):
    sensor_name = sample_data["channel"]
    calibrated_sensor_token = sample_data["calibrated_sensor_token"]
    if calibrated_sensor_token not in logged_sensor_tokens:
        calibrated_sensor = nusc.get("calibrated_sensor", calibrated_sensor_token)
        rotation_xyzw = np.roll(calibrated_sensor["rotation"], shift=-1)
        rr.log(
            f"world/ego_vehicle/{sensor_name}",
            rr.Transform3D(
                translation=calibrated_sensor["translation"],
                rotation=rr.Quaternion(xyzw=rotation_xyzw),
                from_parent=False,
            ),
            timeless=True,
        )
        logged_sensor_tokens.add(calibrated_sensor_token)
        if len(calibrated_sensor["camera_intrinsic"]) != 0:
            rr.log(
                f"world/ego_vehicle/{sensor_name}",
                rr.Pinhole(
                    image_from_camera=calibrated_sensor["camera_intrinsic"],
                    width=sample_data["width"],
                    height=sample_data["height"],
                ),
                timeless=True,
            )

    rr.set_time_seconds("timestamp", (sample_data["timestamp"] - start_timestamp) * 1e-6)

    data_file_path = nusc.dataroot / sample_data["filename"]

    if sample_data["sensor_modality"] == "lidar":
        pointcloud = nuscenes.LidarPointCloud.from_file(str(data_file_path))
        points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
        point_distances = np.linalg.norm(points, axis=1)
        point_colors = cmap(norm(point_distances))
        rr.log(f"world/ego_vehicle/{sensor_name}", rr.Points3D(points, colors=point_colors))

        ego_pose = nusc.get("ego_pose", sample_data["ego_pose_token"])
        rotation_xyzw = np.roll(ego_pose["rotation"], shift=-1)
        rr.log(
            "world/ego_vehicle",
            rr.Transform3D(
                translation=ego_pose["translation"],
                rotation=rr.Quaternion(xyzw=rotation_xyzw),
                from_parent=False,
            ),
        )
    elif sample_data["sensor_modality"] == "radar":
        pointcloud = nuscenes.RadarPointCloud.from_file(str(data_file_path))
        points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
        rr.log(f"world/ego_vehicle/{sensor_name}", rr.Points3D(points))
    elif sample_data["sensor_modality"] == "camera":
        rr.log(f"world/ego_vehicle/{sensor_name}", rr.ImageEncoded(path=data_file_path))


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualizes the nuScenes dataset using the Rerun SDK.")
    parser.add_argument(
        "--root_dir",
        type=pathlib.Path,
        default="dataset",
        help="Root directory of nuScenes dataset",
    )
    parser.add_argument(
        "--scene_name",
        type=str,
        default="scene-0061",
        help="Scene name to visualize (typically of form 'scene-xxxx')",
    )
    parser.add_argument("--dataset_version", type=str, default="v1.0-mini", help="Scene id to visualize")
    rr.script_add_args(parser)
    args = parser.parse_args()

    ensure_scene_available(args.root_dir, args.dataset_version, args.scene_name)

    rr.script_setup(args, "rerun_example_nuscenes")
    log_nuscenes(args.root_dir, args.dataset_version, args.scene_name)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
