from __future__ import annotations

import io
import typing
import zipfile
from argparse import ArgumentParser
from pathlib import Path

import laspy
import numpy as np
import numpy.typing as npt
import requests
import rerun as rr
import rerun.blueprint as rrb
from tqdm import tqdm

DATASET_DIR = Path(__file__).parent / "dataset"
if not DATASET_DIR.exists():
    DATASET_DIR.mkdir()

LIDAR_DATA_FILE = DATASET_DIR / "livemap.las"
TRAJECTORY_DATA_FILE = DATASET_DIR / "livetraj.csv"

LIDAR_DATA_URL = "https://storage.googleapis.com/rerun-example-datasets/flyability/basement/livemap.las.zip"
TRAJECTORY_DATA_URL = "https://storage.googleapis.com/rerun-example-datasets/flyability/basement/livetraj.csv"


def download_with_progress(url: str, what: str) -> io.BytesIO:
    """Download a file with a tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with tqdm(
        desc=f"Downloading {what}",
        total=total_size,
        unit="iB",
        unit_scale=True,
        unit_divisor=1024,
    ) as progress:
        download_file = io.BytesIO()
        for data in resp.iter_content(chunk_size):
            download_file.write(data)
            progress.update(len(data))

    download_file.seek(0)
    return download_file


def unzip_file_from_archive_with_progress(zip_data: typing.BinaryIO, file_name: str, dest_dir: Path) -> None:
    """Unzip the file named `file_name` from the zip archive contained in `zip_data` to `dest_dir`."""
    with zipfile.ZipFile(zip_data, "r") as zip_ref:
        file_info = zip_ref.getinfo(file_name)
        total_size = file_info.file_size

        with tqdm(
            total=total_size,
            desc=f"Extracting file {file_name}",
            unit="iB",
            unit_scale=True,
            unit_divisor=1024,
        ) as progress:
            with zip_ref.open(file_name) as source, open(dest_dir / file_name, "wb") as target:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    target.write(chunk)
                    progress.update(len(chunk))


def download_dataset() -> None:
    if not LIDAR_DATA_FILE.exists():
        unzip_file_from_archive_with_progress(
            download_with_progress(LIDAR_DATA_URL, LIDAR_DATA_FILE.name),
            LIDAR_DATA_FILE.name,
            LIDAR_DATA_FILE.parent,
        )

    if not TRAJECTORY_DATA_FILE.exists():
        TRAJECTORY_DATA_FILE.write_bytes(
            download_with_progress(TRAJECTORY_DATA_URL, TRAJECTORY_DATA_FILE.name).getvalue(),
        )


# TODO(#7333): this utility should be included in the Rerun SDK
def compute_partitions(
    times: npt.NDArray[np.float64],
) -> tuple[typing.Sequence[float], typing.Sequence[np.uintp]]:
    """
    Compute partitions given possibly repeating times.

    This function returns two arrays:
    - Non-repeating times: a filtered version of `times` where repeated times are removed.
    - Partitions: an array of integers where each element indicates the number of elements for the corresponding time
      values in the original `times` array.

    By construction, both arrays should have the same length, and the sum of all elements in `partitions` should be
    equal to the length of `times`.
    """

    change_indices = (np.argwhere(times != np.concatenate([times[1:], np.array([np.nan])])).T + 1).reshape(-1)
    partitions = np.concatenate([[change_indices[0]], np.diff(change_indices)])
    non_repeating_times = times[change_indices - 1]

    assert np.sum(partitions) == len(times)
    assert len(non_repeating_times) == len(partitions)

    return non_repeating_times, partitions  # type: ignore[return-value]


def log_lidar_data() -> None:
    las_data = laspy.read(LIDAR_DATA_FILE)

    # get positions and convert to meters
    points = las_data.points
    positions = np.column_stack((points.X / 1000.0, points.Y / 1000.0, points.Z / 1000.0))
    times = las_data.gps_time

    non_repeating_times, partitions = compute_partitions(times)

    # log all positions at once using the computed partitions
    rr.send_columns(
        "/lidar",
        [rr.TimeColumn("time", duration=non_repeating_times)],
        rr.Points3D.columns(positions=positions).partition(partitions),
    )

    rr.log(
        "/lidar",
        # negative radii are interpreted in UI units (instead of scene units)
        rr.Points3D.from_fields(colors=(128, 128, 255), radii=-0.1),
        static=True,
    )


def log_drone_trajectory() -> None:
    data = np.genfromtxt(TRAJECTORY_DATA_FILE, delimiter=" ", skip_header=1)
    timestamp = data[:, 0]
    positions = data[:, 1:4]

    rr.send_columns(
        "/drone",
        [rr.TimeColumn("time", duration=timestamp)],
        rr.Points3D.columns(positions=positions),
    )

    rr.log(
        "/drone",
        rr.Points3D.from_fields(colors=(255, 0, 0), radii=0.5),
        static=True,
    )


def main() -> None:
    parser = ArgumentParser(description="Visualize drone-based LiDAR data")
    rr.script_add_args(parser)
    args = parser.parse_args()

    download_dataset()

    blueprint = rrb.Spatial3DView(
        origin="/",
        time_ranges=[
            rrb.VisibleTimeRange(
                timeline="time",
                start=rrb.TimeRangeBoundary.cursor_relative(seconds=-60.0),
                end=rrb.TimeRangeBoundary.cursor_relative(),
            ),
        ],
    )

    rr.script_setup(args, "rerun_example_drone_lidar", default_blueprint=blueprint)

    log_lidar_data()
    log_drone_trajectory()


if __name__ == "__main__":
    main()
