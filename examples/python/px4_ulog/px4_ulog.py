from __future__ import annotations

import io
from argparse import ArgumentParser
from pathlib import Path

import numpy as np
import requests
import rerun as rr
import rerun.blueprint as rrb
from pyulog import ULog
from tqdm import tqdm

DATASET_DIR = Path(__file__).parent / "dataset"
if not DATASET_DIR.exists():
    DATASET_DIR.mkdir()

ULOG_FILE_PATH = DATASET_DIR / "log_file.ulg"

ULOG_FILE_URL = "https://github.com/foxglove/ulog/raw/refs/heads/main/tests/log_6_2021-7-20-11-41-56.ulg"


def download_with_progress(url: str, what: str) -> io.BytesIO:
    """Download file with tqdm progress bar."""
    chunk_size = 1024 * 1024
    resp = requests.get(url, stream=True)
    total_size = int(resp.headers.get("content-length", 0))
    with tqdm(
            desc=f"Downloading {what}…",
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


def log_initial_parameters(log_file: ULog) -> None:
    param_table_content = "\n".join([f"| {param} | {value} |" for param, value in log_file.initial_parameters.items()])

    param_text = f"""
## Initial Parameters

| Parameter | Value |
|-----------|-------|
{param_table_content}
"""

    rr.log("initial_parameters", rr.TextDocument(text=param_text, media_type="text/markdown"), static=True)


def log_messages(log_file: ULog) -> None:
    for msg in log_file.logged_messages:
        rr.set_time_seconds("timestamp", msg.timestamp / 1e6)
        rr.log("messages", rr.TextLog(text=msg.message, level=msg.log_level_str()))


def log_data(log_file: ULog) -> None:
    for data in log_file.data_list:
        name = data.name

        columns = data.data.keys() - {"timestamp"}

        time_columns = [rr.TimeSecondsColumn("timestamp", data.data["timestamp"] / 1e6)]

        if "timestamp_sample" in columns:
            time_columns.append(rr.TimeSecondsColumn("timestamp_sample", data.data["timestamp_sample"] / 1e6))
            columns.remove("timestamp_sample")

        all_columns = {}
        for column in columns:
            try:
                col_name, idx = column.split("[")
                idx = int(idx.strip("]"))
            except ValueError:
                col_name = column
                idx = None

            if idx is None:
                all_columns[col_name] = data.data[column]
            else:
                all_columns.setdefault(col_name, {})[idx] = data.data[column]

        for col_name, col_data in all_columns.items():
            entity_path = f"{name}/{col_name.replace('.', '/')}"

            if isinstance(col_data, dict):
                arr = np.vstack([col_data[i] for i in sorted(col_data.keys())]).T
            else:
                arr = col_data

            rr.send_columns(entity_path, indexes=time_columns, columns=rr.Scalar.columns(scalar=arr))

        if name == "vehicle_global_position":
            latlon = np.vstack([data.data["lat"], data.data["lon"]]).T
            rr.send_columns(name, indexes=time_columns, columns=rr.GeoPoints.columns(positions=latlon))
        elif name == "vehicle_local_position":
            pos3d = np.vstack([data.data["x"], data.data["y"], -data.data["z"]]).T
            pos2d = np.vstack([data.data["x"], data.data["y"]]).T
            rr.send_columns(
                f"{name}/pos",
                indexes=time_columns,
                columns=[
                    *rr.Points3D.columns(positions=pos3d),
                    *rr.Points2D.columns(positions=pos2d),
                ],
            )

            v2d = np.vstack([data.data["vx"], data.data["vy"]]).T
            v3d = np.vstack([data.data["vx"], data.data["vy"], -data.data["vz"]]).T
            rr.send_columns(
                f"{name}/vel",
                indexes=time_columns,
                columns=[
                    *rr.Arrows3D.columns(origins=pos3d, vectors=v3d),
                    *rr.Arrows2D.columns(origins=pos2d, vectors=v2d),
                ],
            )
        elif name == "position_setpoint_triplet":
            latlon = np.vstack([data.data["next.lat"], data.data["next.lon"]]).T
            radius = data.data["next.loiter_radius"]

            rr.send_columns(
                f"{name}/next/latlon",
                indexes=time_columns,
                columns=rr.GeoPoints.columns(positions=latlon, radii=radius)
            )


def main() -> None:
    parser = ArgumentParser(description="Loada and visualize a PX4 ulog file")

    parser.add_argument("--uuid", type=str, help="UUID of log file on `review.px4.io`")

    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_px4_log")

    if args.uuid is not None:
        data = download_with_progress(f"https://review.px4.io/download?log={args.uuid}", "ulog file")
        file_path = DATASET_DIR / f"{args.uuid}.ulg"
        file_path.write_bytes(data.read())
    else:
        file_path = ULOG_FILE_PATH
        if not file_path.exists():
            ulog_file_data = download_with_progress(ULOG_FILE_URL, "ulog file")
            file_path.write_bytes(ulog_file_data.read())

    log_file = ULog(str(file_path))
    log_initial_parameters(log_file)
    log_messages(log_file)
    log_data(log_file)

    blueprint = rrb.Vertical(
        rrb.Horizontal(
            rrb.Tabs(
                rrb.Spatial2DView(
                    name="2D",
                    overrides={
                        "vehicle_local_position/pos": [
                            # TODO: broken because of #8557
                            rrb.VisibleTimeRange(
                                "timestamp",
                                start=rrb.TimeRangeBoundary.infinite(),
                                end=rrb.TimeRangeBoundary.infinite(),
                            )
                        ],
                    },
                ),
                rrb.Spatial3DView(
                    name="3D",
                    overrides={
                        "vehicle_local_position/pos": [
                            rrb.VisibleTimeRange(
                                "timestamp",
                                start=rrb.TimeRangeBoundary.infinite(),
                                end=rrb.TimeRangeBoundary.infinite(),
                            )
                        ],
                    },
                ),
                rrb.MapView(name="Map"),
            ),
            rrb.TextLogView(),
            column_shares=[3, 1],
        ),
        rrb.Tabs(*[rrb.TimeSeriesView(origin=data.name) for data in log_file.data_list]),
    )

    rr.send_blueprint(blueprint, make_active=False, make_default=True)


if __name__ == "__main__":
    main()
