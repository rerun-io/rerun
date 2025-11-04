#!/usr/bin/env python3
"""
Load an Open Photogrammetry Format (OPF) project and display the cameras and point cloud.

Requires Python 3.10 or higher because of [pyopf](https://pypi.org/project/pyopf/).
"""

from __future__ import annotations

import argparse
import logging
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Final

import numpy as np
import requests
import rerun as rr
import tqdm
from PIL import Image
from pyopf.io import load
from pyopf.resolve import resolve

DESCRIPTION = """
# Open Photogrammetry Format

Visualizes an Open Photogrammetry Format (OPF) project, displaying the cameras and point cloud.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/open_photogrammetry_format).

### Links
* [OPF specification](https://pix4d.github.io/opf-spec/index.html)
* [Dataset source](https://support.pix4d.com/hc/en-us/articles/360000235126#OPF)
* [pyopf](https://github.com/Pix4D/pyopf)
"""


@dataclass
class DatasetSpec:
    dir_name: str
    url: str


DATASETS = {
    "olympic": DatasetSpec("olympic_flame", "https://data.pix4d.com/misc/example_datasets/olympic_flame.zip"),
    "rainwater": DatasetSpec(
        "catch_rainwater_demo",
        "https://data.pix4d.com/misc/example_datasets/catch_rainwater_demo.zip",
    ),
    "rivaz": DatasetSpec("rivaz_demo", "https://data.pix4d.com/misc/example_datasets/rivaz_demo.zip"),
}
DATASET_DIR: Final = Path(__file__).parent / "dataset"


def download_file(url: str, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    logging.info("Downloading %s to %s", url, path)
    response = requests.get(url, stream=True)
    with tqdm.tqdm.wrapattr(
        open(path, "wb"),
        "write",
        miniters=1,
        total=int(response.headers.get("content-length", 0)),
        desc=f"Downloading {path.name}",
    ) as f:
        for chunk in response.iter_content(chunk_size=4096):
            f.write(chunk)


def unzip_dir(archive: Path, destination: Path) -> None:
    """Unzip the archive to the destination, using tqdm to display progress."""
    logging.info("Extracting %s to %s", archive, destination)
    with zipfile.ZipFile(archive, "r") as zip_ref:
        zip_ref.extractall(destination)


class OPFProject:
    def __init__(self, path: Path, log_as_frames: bool = True) -> None:
        """
        Create a new OPFProject from the given path.

        Parameters
        ----------
        path : Path
            Path to the project file.
        log_as_frames : bool, optional
            Whether to log the cameras as individual frames, by default True

        """
        self.path = path
        self.project = resolve(load(str(self.path)))
        self.log_as_frames = log_as_frames

    @classmethod
    def from_dataset(cls, dataset: str, log_as_frames: bool = True) -> OPFProject:
        """
        Download the dataset if necessary and return the project file.

        Parameters
        ----------
        dataset : str
            Name of the dataset to download.
        log_as_frames : bool, optional
            Whether to log the cameras as individual frames, by default True

        """
        spec = DATASETS[dataset]
        if not (DATASET_DIR / spec.dir_name).exists():
            zip_file = DATASET_DIR / f"{dataset}.zip"
            if not zip_file.exists():
                download_file(DATASETS[dataset].url, zip_file)
            unzip_dir(DATASET_DIR / f"{dataset}.zip", DATASET_DIR)

        return cls(DATASET_DIR / spec.dir_name / "project.opf", log_as_frames=log_as_frames)

    def log_point_cloud(self) -> None:
        """Log the project's point cloud."""
        points = self.project.point_cloud_objs[0].nodes[0]
        rr.log("world/points", rr.Points3D(points.position, colors=points.color), static=True)

    def log_calibrated_cameras(self, jpeg_quality: int | None) -> None:
        """
        Log the project's calibrated cameras as individual frames.

        Logging all cameras in a single frame is also possible, but clutter the default view with too many image views.
        """
        sensor_map = {sensor.id: sensor for sensor in self.project.input_cameras.sensors}
        calib_sensor_map = {sensor.id: sensor for sensor in self.project.calibration.calibrated_cameras.sensors}

        for i, (camera, calib_camera) in enumerate(
            zip(
                self.project.camera_list.cameras,
                self.project.calibration.calibrated_cameras.cameras,
                strict=False,
            ),
        ):
            if not str(camera.uri).endswith(".jpg"):
                continue

            if self.log_as_frames:
                rr.set_time("image", sequence=i)
                entity = "world/cameras"
            else:
                entity = f"world/cameras/{i}"

            sensor = sensor_map[calib_camera.sensor_id]
            calib_sensor = calib_sensor_map[calib_camera.sensor_id]

            # Specification for the omega, phi, kappa angles:
            # https://pix4d.github.io/opf-spec/specification/calibrated_cameras.html#calibrated-camera
            omega, phi, kappa = tuple(np.deg2rad(a) for a in calib_camera.orientation_deg)
            rot = (
                np.array([
                    [1, 0, 0],
                    [0, np.cos(omega), -np.sin(omega)],
                    [0, np.sin(omega), np.cos(omega)],
                ])
                @ np.array([
                    [np.cos(phi), 0, np.sin(phi)],
                    [0, 1, 0],
                    [-np.sin(phi), 0, np.cos(phi)],
                ])
                @ np.array([
                    [np.cos(kappa), -np.sin(kappa), 0],
                    [np.sin(kappa), np.cos(kappa), 0],
                    [0, 0, 1],
                ])
            )

            rr.log(entity, rr.Transform3D(translation=calib_camera.position, mat3x3=rot))

            assert calib_sensor.internals.type == "perspective"

            # RUB coordinate system specified in https://pix4d.github.io/opf-spec/specification/projected_input_cameras.html#coordinate-system-specification
            rr.log(
                entity + "/image",
                rr.Pinhole(
                    resolution=sensor.image_size_px,
                    focal_length=calib_sensor.internals.focal_length_px,
                    principal_point=calib_sensor.internals.principal_point_px,
                    camera_xyz=rr.ViewCoordinates.RUB,
                ),
            )

            if jpeg_quality is not None:
                with Image.open(self.path.parent / camera.uri) as img:
                    rr.log(entity + "/image/rgb", rr.Image(img).compress(jpeg_quality=jpeg_quality))
            else:
                rr.log(entity + "/image/rgb", rr.EncodedImage(path=self.path.parent / camera.uri))


def main() -> None:
    logging.getLogger().addHandler(rr.LoggingHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(
        description="Load an Open Photogrammetry Format (OPF) project and display the cameras and point cloud.",
    )
    parser.add_argument(
        "--dataset",
        choices=DATASETS.keys(),
        default="olympic",
        help="Run on a demo image automatically downloaded",
    )
    parser.add_argument(
        "--no-frames",
        action="store_true",
        help="Log all cameras globally instead of as individual frames in the timeline.",
    )
    parser.add_argument(
        "--jpeg-quality",
        type=int,
        default=None,
        help="If specified, compress the camera images with the given JPEG quality.",
    )

    rr.script_add_args(parser)

    args, unknown = parser.parse_known_args()
    for arg in unknown:
        logging.warning(f"unknown arg: {arg}")

    # load the data set
    project = OPFProject.from_dataset(args.dataset, log_as_frames=not args.no_frames)

    # display everything in Rerun
    rr.script_setup(args, "rerun_example_open_photogrammetry_format")
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)
    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)
    project.log_point_cloud()
    project.log_calibrated_cameras(jpeg_quality=args.jpeg_quality)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
