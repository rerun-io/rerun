#!/usr/bin/env python3
from __future__ import annotations

import argparse
import pathlib
import sys
from typing import Any, Final

import matplotlib
import numpy as np
import rerun as rr
import rerun.blueprint as rrb
from nuscenes import nuscenes

from .download_dataset import MINISPLIT_SCENES, download_minisplit
from .export_gps import derive_latlon

DESCRIPTION = """
# nuScenes

Visualize the [nuScenes dataset](https://www.nuscenes.org/) including lidar, radar, images, and bounding boxes data.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/nuscenes_dataset).
"""

EXAMPLE_DIR: Final = pathlib.Path(__file__).parent.parent
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"

# currently need to calculate the color manually
# see https://github.com/rerun-io/rerun/issues/4409
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
            sys.exit()

    scene_names = [s["name"] for s in nusc.scene]
    if scene_name not in scene_names:
        raise ValueError(f"{scene_name=} not found in dataset")


def nuscene_sensor_names(nusc: nuscenes.NuScenes, scene_name: str) -> list[str]:
    """Return all sensor names in the scene."""

    sensor_names = set()

    scene = next(s for s in nusc.scene if s["name"] == scene_name)
    first_sample = nusc.get("sample", scene["first_sample_token"])
    for sample_data_token in first_sample["data"].values():
        sample_data = nusc.get("sample_data", sample_data_token)
        if sample_data["sensor_modality"] == "camera":
            current_camera_token = sample_data_token
            while current_camera_token != "":
                sample_data = nusc.get("sample_data", current_camera_token)
                sensor_name = sample_data["channel"]
                sensor_names.add(sensor_name)
                current_camera_token = sample_data["next"]

    # For a known set of cameras, order the sensors in a circle.
    ordering = {
        "CAM_FRONT_LEFT": 0,
        "CAM_FRONT": 1,
        "CAM_FRONT_RIGHT": 2,
        "CAM_BACK_RIGHT": 3,
        "CAM_BACK": 4,
        "CAM_BACK_LEFT": 5,
    }
    return sorted(sensor_names, key=lambda sensor_name: ordering.get(sensor_name, float("inf")))


def log_nuscenes(nusc: nuscenes.NuScenes, scene_name: str, max_time_sec: float) -> None:
    """Log nuScenes scene."""

    scene = next(s for s in nusc.scene if s["name"] == scene_name)

    location = nusc.get("log", scene["log_token"])["location"]

    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

    first_sample_token = scene["first_sample_token"]
    first_sample = nusc.get("sample", scene["first_sample_token"])

    first_lidar_token = ""
    first_radar_tokens = []
    first_camera_tokens = []
    for sample_data_token in first_sample["data"].values():
        sample_data = nusc.get("sample_data", sample_data_token)
        log_sensor_calibration(sample_data, nusc)

        if sample_data["sensor_modality"] == "lidar":
            first_lidar_token = sample_data_token
        elif sample_data["sensor_modality"] == "radar":
            first_radar_tokens.append(sample_data_token)
        elif sample_data["sensor_modality"] == "camera":
            first_camera_tokens.append(sample_data_token)

    first_timestamp_us = nusc.get("sample_data", first_lidar_token)["timestamp"]
    max_timestamp_us = first_timestamp_us + 1e6 * max_time_sec

    log_lidar_and_ego_pose(location, first_lidar_token, nusc, max_timestamp_us)
    log_cameras(first_camera_tokens, nusc, max_timestamp_us)
    log_radars(first_radar_tokens, nusc, max_timestamp_us)
    log_annotations(location, first_sample_token, nusc, max_timestamp_us)


def log_lidar_and_ego_pose(
    location: str,
    first_lidar_token: str,
    nusc: nuscenes.NuScenes,
    max_timestamp_us: float,
) -> None:
    """Log lidar data and vehicle pose."""
    current_lidar_token = first_lidar_token

    ego_trajectory_lat_lon = []

    while current_lidar_token != "":
        sample_data = nusc.get("sample_data", current_lidar_token)
        sensor_name = sample_data["channel"]

        if max_timestamp_us < sample_data["timestamp"]:
            break

        # timestamps are in microseconds
        rr.set_time("timestamp", timestamp=sample_data["timestamp"] * 1e-6)

        ego_pose = nusc.get("ego_pose", sample_data["ego_pose_token"])
        rotation_xyzw = np.roll(ego_pose["rotation"], shift=-1)  # go from wxyz to xyzw
        position_lat_lon = derive_latlon(location, ego_pose)
        ego_trajectory_lat_lon.append(position_lat_lon)

        rr.log(
            "world/ego_vehicle",
            rr.Transform3D(
                translation=ego_pose["translation"],
                rotation=rr.Quaternion(xyzw=rotation_xyzw),
                relation=rr.TransformRelation.ParentFromChild,
            ),
            rr.TransformAxes3D(10.0),  # The length of the visualized axis.
            rr.GeoPoints(lat_lon=position_lat_lon, radii=rr.Radius.ui_points(8.0), colors=0xFF0000FF),
        )
        # TODO(#10632): We don't want the radius for the trajectory line to be the same as the radius of the points.
        # However, rr.GeoPoints uses the same `rr.components.Radius` for this, so these two archetypes would influence each other
        # if logged on the same entity. In the future, they will have different tags, which will allow them to live side by side.
        rr.log(
            "world/ego_vehicle/trajectory",
            rr.GeoLineStrings(lat_lon=ego_trajectory_lat_lon, radii=rr.Radius.ui_points(1.0), colors=0xFF0000FF),
        )

        current_lidar_token = sample_data["next"]

        data_file_path = nusc.dataroot / sample_data["filename"]
        pointcloud = nuscenes.LidarPointCloud.from_file(str(data_file_path))
        points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
        point_distances = np.linalg.norm(points, axis=1)
        point_colors = cmap(norm(point_distances))
        rr.log(f"world/ego_vehicle/{sensor_name}", rr.Points3D(points, colors=point_colors))


def log_cameras(first_camera_tokens: list[str], nusc: nuscenes.NuScenes, max_timestamp_us: float) -> None:
    """Log camera data."""
    for first_camera_token in first_camera_tokens:
        current_camera_token = first_camera_token
        while current_camera_token != "":
            sample_data = nusc.get("sample_data", current_camera_token)
            if max_timestamp_us < sample_data["timestamp"]:
                break
            sensor_name = sample_data["channel"]
            rr.set_time("timestamp", timestamp=sample_data["timestamp"] * 1e-6)
            data_file_path = nusc.dataroot / sample_data["filename"]
            rr.log(f"world/ego_vehicle/{sensor_name}", rr.EncodedImage(path=data_file_path))
            current_camera_token = sample_data["next"]


def log_radars(first_radar_tokens: list[str], nusc: nuscenes.NuScenes, max_timestamp_us: float) -> None:
    """Log radar data."""
    for first_radar_token in first_radar_tokens:
        current_camera_token = first_radar_token
        while current_camera_token != "":
            sample_data = nusc.get("sample_data", current_camera_token)
            if max_timestamp_us < sample_data["timestamp"]:
                break
            sensor_name = sample_data["channel"]
            rr.set_time("timestamp", timestamp=sample_data["timestamp"] * 1e-6)
            data_file_path = nusc.dataroot / sample_data["filename"]
            pointcloud = nuscenes.RadarPointCloud.from_file(str(data_file_path))
            points = pointcloud.points[:3].T  # shape after transposing: (num_points, 3)
            point_distances = np.linalg.norm(points, axis=1)
            point_colors = cmap(norm(point_distances))
            rr.log(
                f"world/ego_vehicle/{sensor_name}",
                rr.Points3D(points, colors=point_colors),
            )
            current_camera_token = sample_data["next"]


def log_annotations(location: str, first_sample_token: str, nusc: nuscenes.NuScenes, max_timestamp_us: float) -> None:
    """Log 3D bounding boxes."""
    label2id: dict[str, int] = {}
    current_sample_token = first_sample_token
    while current_sample_token != "":
        sample_data = nusc.get("sample", current_sample_token)
        if max_timestamp_us < sample_data["timestamp"]:
            break
        rr.set_time("timestamp", timestamp=sample_data["timestamp"] * 1e-6)
        ann_tokens = sample_data["anns"]
        sizes = []
        centers = []
        quaternions = []
        class_ids = []
        lat_lon = []
        for ann_token in ann_tokens:
            ann = nusc.get("sample_annotation", ann_token)

            rotation_xyzw = np.roll(ann["rotation"], shift=-1)  # go from wxyz to xyzw
            width, length, height = ann["size"]
            sizes.append((length, width, height))  # x, y, z sizes
            centers.append(ann["translation"])
            quaternions.append(rr.Quaternion(xyzw=rotation_xyzw))
            if ann["category_name"] not in label2id:
                label2id[ann["category_name"]] = len(label2id)
            class_ids.append(label2id[ann["category_name"]])
            lat_lon.append(derive_latlon(location, ann))

        rr.log(
            "world/anns",
            rr.Boxes3D(
                sizes=sizes,
                centers=centers,
                quaternions=quaternions,
                class_ids=class_ids,
            ),
            rr.GeoPoints(lat_lon=lat_lon),
        )
        current_sample_token = sample_data["next"]

    annotation_context = [(i, label) for label, i in label2id.items()]
    rr.log("world/anns", rr.AnnotationContext(annotation_context), static=True)


def log_sensor_calibration(sample_data: dict[str, Any], nusc: nuscenes.NuScenes) -> None:
    """Log sensor calibration (pinhole camera, sensor poses, etc.)."""
    sensor_name = sample_data["channel"]
    calibrated_sensor_token = sample_data["calibrated_sensor_token"]
    calibrated_sensor = nusc.get("calibrated_sensor", calibrated_sensor_token)
    rotation_xyzw = np.roll(calibrated_sensor["rotation"], shift=-1)  # go from wxyz to xyzw
    rr.log(
        f"world/ego_vehicle/{sensor_name}",
        rr.Transform3D(
            translation=calibrated_sensor["translation"],
            rotation=rr.Quaternion(xyzw=rotation_xyzw),
            relation=rr.TransformRelation.ParentFromChild,
        ),
        static=True,
    )
    if len(calibrated_sensor["camera_intrinsic"]) != 0:
        rr.log(
            f"world/ego_vehicle/{sensor_name}",
            rr.Pinhole(
                image_from_camera=calibrated_sensor["camera_intrinsic"],
                width=sample_data["width"],
                height=sample_data["height"],
            ),
            static=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualizes the nuScenes dataset using the Rerun SDK.")
    parser.add_argument(
        "--root-dir",
        type=pathlib.Path,
        default=DATASET_DIR,
        help="Root directory of nuScenes dataset",
    )
    parser.add_argument(
        "--scene-name",
        type=str,
        default="scene-0061",
        help="Scene name to visualize (typically of form 'scene-xxxx')",
    )
    parser.add_argument("--dataset-version", type=str, default="v1.0-mini", help="Scene id to visualize")
    parser.add_argument(
        "--seconds",
        type=float,
        default=float("inf"),
        help="If specified, limits the number of seconds logged",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    ensure_scene_available(args.root_dir, args.dataset_version, args.scene_name)

    nusc = nuscenes.NuScenes(version=args.dataset_version, dataroot=args.root_dir, verbose=True)

    # Set up the Rerun Blueprint (how the visualization is organized):
    sensor_views = [
        rrb.Spatial2DView(
            name=sensor_name,
            origin=f"world/ego_vehicle/{sensor_name}",
            contents=["$origin/**", "world/anns"],
            overrides={"world/anns": rr.Boxes3D.from_fields(fill_mode="majorwireframe")},
        )
        for sensor_name in nuscene_sensor_names(nusc, args.scene_name)
    ]
    blueprint = rrb.Blueprint(
        rrb.Vertical(
            rrb.Horizontal(
                rrb.Spatial3DView(
                    name="3D",
                    origin="world",
                    # Set the image plane distance to 5m for all camera visualizations.
                    defaults=[rr.Pinhole.from_fields(image_plane_distance=5.0)],
                    overrides={"world/anns": rr.Boxes3D(fill_mode="solid")},
                ),
                rrb.Vertical(
                    rrb.TextDocumentView(origin="description", name="Description"),
                    rrb.MapView(
                        origin="world",
                        name="MapView",
                        zoom=rrb.archetypes.MapZoom(18.0),
                        background=rrb.archetypes.MapBackground(rrb.components.MapProvider.OpenStreetMap),
                    ),
                    row_shares=[1, 1],
                ),
                column_shares=[3, 1],
            ),
            rrb.Grid(*sensor_views),
            row_shares=[4, 2],
        ),
        rrb.TimePanel(state="collapsed"),
    )

    rr.script_setup(args, "rerun_example_nuscenes", default_blueprint=blueprint)

    rr.log(
        "description",
        rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN),
        static=True,
    )

    log_nuscenes(nusc, args.scene_name, max_time_sec=args.seconds)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
