from __future__ import annotations

import os
import pathlib
import tarfile

import pandas as pd
import requests
import rerun as rr
from rerun import blueprint as rrb

cwd = pathlib.Path(__file__).parent.resolve()

DATASET_URL = "https://vision.in.tum.de/tumvi/exported/euroc/512_16/dataset-corridor4_512_16.tar"
DATASET_NAME = "dataset-corridor4_512_16"
XYZ_AXIS_NAMES = ["x", "y", "z"]
XYZ_AXIS_COLORS = [[(231, 76, 60), (39, 174, 96), (52, 120, 219)]]


def main() -> None:
    dataset_path = cwd / DATASET_NAME
    if not dataset_path.exists():
        _download_dataset(cwd)

    _setup_rerun()
    _log_imu_data()
    _log_image_data()
    _log_gt_imu()


def _download_dataset(root: pathlib.Path, dataset_url: str = DATASET_URL) -> None:
    os.makedirs(root, exist_ok=True)
    tar_path = os.path.join(root, "dataset-corridor4_512_16.tar")
    print("Downloading dataset...")
    with requests.get(dataset_url, stream=True) as r:
        r.raise_for_status()
        with open(tar_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)
    print("Extracting dataset...")
    with tarfile.open(tar_path, "r:") as tar:
        tar.extractall(path=root)
    os.remove(tar_path)


def _setup_rerun() -> None:
    rr.init("rerun_example_imu_data", spawn=True)

    rr.send_blueprint(
        rrb.Horizontal(
            rrb.Vertical(
                rrb.TimeSeriesView(
                    origin="gyroscope",
                    name="Gyroscope",
                    overrides={
                        # TODO(#9022): Pluralize series line type.
                        "/gyroscope": rr.SeriesLine.from_fields(name=XYZ_AXIS_NAMES, color=XYZ_AXIS_COLORS),  # type: ignore[arg-type]
                    },
                ),
                rrb.TimeSeriesView(
                    origin="accelerometer",
                    name="Accelerometer",
                    overrides={
                        # TODO(#9022): Pluralize series line type.
                        "/accelerometer": rr.SeriesLine.from_fields(name=XYZ_AXIS_NAMES, color=XYZ_AXIS_COLORS),  # type: ignore[arg-type]
                    },
                ),
            ),
            rrb.Spatial3DView(origin="/", name="World position"),
            column_shares=[0.45, 0.55],
        ),
    )


def _log_imu_data() -> None:
    imu_data = pd.read_csv(
        cwd / DATASET_NAME / "dso/imu.txt",
        sep=" ",
        header=0,
        names=["timestamp", "gyro.x", "gyro.y", "gyro.z", "accel.x", "accel.y", "accel.z"],
        comment="#",
    )

    times = rr.TimeColumn("timestamp", datetime=imu_data["timestamp"])

    gyro = imu_data[["gyro.x", "gyro.y", "gyro.z"]]
    rr.send_columns("/gyroscope", indexes=[times], columns=rr.Scalar.columns(scalar=gyro))

    accel = imu_data[["accel.x", "accel.y", "accel.z"]]
    rr.send_columns("/accelerometer", indexes=[times], columns=rr.Scalar.columns(scalar=accel))


def _log_image_data() -> None:
    times = pd.read_csv(
        cwd / DATASET_NAME / "dso/cam0/times.txt",
        sep=" ",
        header=0,
        names=["filename", "timestamp", "exposure_time"],
        comment="#",
        dtype={"filename": str},
    )

    rr.set_time("timestamp", datetime=times["timestamp"][0])
    rr.log(
        "/cam0",
        rr.Pinhole(
            focal_length=(0.373 * 512, 0.373 * 512),
            resolution=(512, 512),
            camera_xyz=rr.components.ViewCoordinates.FLU,
            image_plane_distance=0.4,
        ),
        static=True,
    )

    for _, (filename, timestamp, _) in times.iterrows():
        image_path = cwd / DATASET_NAME / "dso/cam0/images" / f"{filename}.png"
        rr.set_time("timestamp", datetime=timestamp)
        rr.log("/cam0/image", rr.ImageEncoded(path=image_path))


def _log_gt_imu() -> None:
    gt_imu = pd.read_csv(
        cwd / DATASET_NAME / "dso/gt_imu.csv",
        sep=",",
        header=0,
        names=["timestamp", "t.x", "t.y", "t.z", "q.w", "q.x", "q.y", "q.z"],
        comment="#",
    )

    times = rr.TimeColumn("timestamp", datetime=gt_imu["timestamp"])

    translations = gt_imu[["t.x", "t.y", "t.z"]]
    quaternions = gt_imu[
        [
            "q.x",
            "q.y",
            "q.z",
            "q.w",
        ]
    ]
    rr.send_columns(
        "/cam0",
        indexes=[times],
        columns=rr.Transform3D.columns(translation=translations, quaternion=quaternions),
    )


if __name__ == "__main__":
    main()
