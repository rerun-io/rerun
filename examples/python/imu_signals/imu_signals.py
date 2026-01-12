from __future__ import annotations

import argparse
import os
import tarfile
from pathlib import Path

import numpy as np
import pandas as pd
import requests
import rerun as rr
from rerun import blueprint as rrb
from tqdm.auto import tqdm

DATA_DIR = Path(__file__).parent / "dataset"

DATASET_URL = "https://storage.googleapis.com/rerun-example-datasets/imu_signals/tum_vi_corridor4_512_16.tar"
DATASET_NAME = "dataset-corridor4_512_16"
XYZ_AXIS_NAMES = ["x", "y", "z"]
XYZ_AXIS_COLORS = [[(231, 76, 60), (39, 174, 96), (52, 120, 219)]]


def main() -> None:
    dataset_path = DATA_DIR / DATASET_NAME
    if not dataset_path.exists():
        _download_dataset(DATA_DIR)

    parser = argparse.ArgumentParser(description="Visualizes the TUM Visual-Inertial dataset using the Rerun SDK.")
    parser.add_argument(
        "--seconds",
        type=float,
        default=float("inf"),
        help="If specified, limits the number of seconds logged",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Horizontal(
        rrb.Vertical(
            rrb.TimeSeriesView(
                origin="gyroscope",
                name="Gyroscope",
                overrides={"/gyroscope": rr.SeriesLines.from_fields(names=XYZ_AXIS_NAMES, colors=XYZ_AXIS_COLORS)},
            ),
            rrb.TimeSeriesView(
                origin="accelerometer",
                name="Accelerometer",
                overrides={"/accelerometer": rr.SeriesLines.from_fields(names=XYZ_AXIS_NAMES, colors=XYZ_AXIS_COLORS)},
            ),
        ),
        rrb.Spatial3DView(origin="/", name="World position"),
        column_shares=[0.45, 0.55],
    )

    rr.script_setup(args, "rerun_example_imu_signals", default_blueprint=blueprint)

    _log_imu_data(args.seconds)
    _log_image_data(args.seconds)
    _log_gt_imu(args.seconds)


def _download_dataset(root: Path, dataset_url: str = DATASET_URL) -> None:
    os.makedirs(root, exist_ok=True)
    tar_path = os.path.join(root, f"{DATASET_NAME}.tar")
    response = requests.get(dataset_url, stream=True)

    total_size = int(response.headers.get("content-length", 0))
    block_size = 1024

    with tqdm(desc="Downloading dataset", total=total_size, unit="B", unit_scale=True) as pb:
        with open(tar_path, "wb") as file:
            for data in response.iter_content(chunk_size=block_size):
                pb.update(len(data))
                file.write(data)

    if total_size not in (0, pb.n):
        raise RuntimeError("Failed to download complete dataset!")

    print("Extracting dataset…")
    with tarfile.open(tar_path, "r:") as tar:
        tar.extractall(path=root)
    os.remove(tar_path)


def _log_imu_data(max_time_sec: float) -> None:
    imu_data = pd.read_csv(
        DATA_DIR / DATASET_NAME / "dso/imu.txt",
        sep=" ",
        header=0,
        names=["timestamp", "gyro.x", "gyro.y", "gyro.z", "accel.x", "accel.y", "accel.z"],
        comment="#",
    )

    timestamps = imu_data["timestamp"].to_numpy()
    max_time_ns = imu_data["timestamp"][0] + max_time_sec * 1e9
    selected = imu_data[imu_data["timestamp"] <= max_time_ns]

    timestamps = selected["timestamp"].astype("datetime64[ns]")
    times = rr.TimeColumn("timestamp", timestamp=timestamps)

    gyro = selected[["gyro.x", "gyro.y", "gyro.z"]].to_numpy()
    rr.send_columns("/gyroscope", indexes=[times], columns=rr.Scalars.columns(scalars=gyro))

    accel = selected[["accel.x", "accel.y", "accel.z"]]
    rr.send_columns("/accelerometer", indexes=[times], columns=rr.Scalars.columns(scalars=accel))


def _log_image_data(max_time_sec: float) -> None:
    times = pd.read_csv(
        DATA_DIR / DATASET_NAME / "dso/cam0/times.txt",
        sep=" ",
        header=0,
        names=["filename", "timestamp", "exposure_time"],
        comment="#",
        dtype={"filename": str},
    )

    rr.set_time("timestamp", timestamp=times["timestamp"][0])
    rr.log(
        "/world",
        rr.Transform3D(rotation_axis_angle=rr.RotationAxisAngle(axis=(1, 0, 0), angle=-np.pi / 2)),
        static=True,
    )
    rr.log(
        "/world/cam0",
        rr.Pinhole(
            focal_length=(0.373 * 512, 0.373 * 512),
            resolution=(512, 512),
            image_plane_distance=0.4,
        ),
        static=True,
    )

    max_time_sec = times["timestamp"][0] + max_time_sec
    for _, (filename, timestamp, _) in times.iterrows():
        if timestamp > max_time_sec:
            break

        image_path = DATA_DIR / DATASET_NAME / "dso/cam0/images" / f"{filename}.png"
        rr.set_time("timestamp", timestamp=timestamp)
        rr.log("/world/cam0/image", rr.EncodedImage(path=image_path))


def _log_gt_imu(max_time_sec: float) -> None:
    gt_imu = pd.read_csv(
        DATA_DIR / DATASET_NAME / "dso/gt_imu.csv",
        sep=",",
        header=0,
        names=["timestamp", "t.x", "t.y", "t.z", "q.w", "q.x", "q.y", "q.z"],
        comment="#",
    )

    timestamps = gt_imu["timestamp"].to_numpy()
    max_time_ns = gt_imu["timestamp"][0] + max_time_sec * 1e9
    selected = gt_imu[gt_imu["timestamp"] <= max_time_ns]

    timestamps = selected["timestamp"].astype("datetime64[ns]")
    times = rr.TimeColumn("timestamp", timestamp=timestamps)

    translations = selected[["t.x", "t.y", "t.z"]]
    quaternions = selected[
        [
            "q.x",
            "q.y",
            "q.z",
            "q.w",
        ]
    ]
    rr.send_columns(
        "/world/cam0",
        indexes=[times],
        columns=rr.Transform3D.columns(
            translation=translations,
            quaternion=quaternions,
        ),
    )


if __name__ == "__main__":
    main()
