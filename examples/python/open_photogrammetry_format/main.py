"""
Load an Open Photogrammetry Format (OFP) project and display the cameras and point cloud.

OPF specification: https://pix4d.github.io/opf-spec/index.html
Dataset source: https://support.pix4d.com/hc/en-us/articles/360000235126-Example-projects-real-photogrammetry-data#OPF1
pyopf: https://github.com/Pix4D/pyopf
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
from pyopf.io import load
from pyopf.resolve import resolve


@dataclass
class DatasetSpec:
    dir_name: str
    url: str


DATASETS = {
    "olympic": DatasetSpec(
        "olympic_flame", "https://s3.amazonaws.com/mics.pix4d.com/example_datasets/olympic_flame.zip"
    ),
    "rainwater": DatasetSpec(
        "catch_rainwater_demo", "https://s3.amazonaws.com/mics.pix4d.com/example_datasets/catch_rainwater_demo.zip"
    ),
    "rivaz": DatasetSpec("rivaz_demo", "https://s3.amazonaws.com/mics.pix4d.com/example_datasets/rivaz_demo.zip"),
}

# Path to the example project file.
PROJECT_DIR = Path("/Users/hhip/Downloads/rivaz_demo")
EXAMPLE_DIR: Final = Path(__file__).parent
DATASET_DIR: Final = EXAMPLE_DIR / "dataset"


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
    def __init__(self, path: Path) -> None:
        self.path = path
        self.project = resolve(load(str(path)))

    @classmethod
    def from_dataset(cls, dataset: str) -> "OPFProject":
        """Download the dataset if necessary and return the project file."""
        spec = DATASETS[dataset]
        if not (DATASET_DIR / spec.dir_name).exists():
            zip_file = DATASET_DIR / f"{dataset}.zip"
            if not zip_file.exists():
                download_file(DATASETS[dataset].url, zip_file)
            unzip_dir(DATASET_DIR / f"{dataset}.zip", DATASET_DIR)

        return cls(DATASET_DIR / spec.dir_name / "project.opf")

    def log_point_cloud(self) -> None:
        """Log the project's point cloud."""
        pcl = self.project.point_cloud_objs[0]
        rr.log_points("world/pcl", positions=pcl.nodes[0].position, colors=pcl.nodes[0].color, timeless=True)

    def log_cameras_as_frames(self) -> None:
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
            )
        ):
            if not str(camera.uri).endswith(".jpg"):
                continue

            rr.set_time_sequence("image", i)
            entity = "world/cameras"

            sensor = sensor_map[calib_camera.sensor_id]
            calib_sensor = calib_sensor_map[calib_camera.sensor_id]

            omega, phi, kappa = tuple(np.deg2rad(a) for a in calib_camera.orientation_deg)
            rot = (
                np.array(
                    [
                        [1, 0, 0],
                        [0, np.cos(omega), -np.sin(omega)],
                        [0, np.sin(omega), np.cos(omega)],
                    ]
                )
                @ np.array(
                    [
                        [np.cos(phi), 0, np.sin(phi)],
                        [0, 1, 0],
                        [-np.sin(phi), 0, np.cos(phi)],
                    ]
                )
                @ np.array(
                    [
                        [np.cos(kappa), -np.sin(kappa), 0],
                        [np.sin(kappa), np.cos(kappa), 0],
                        [0, 0, 1],
                    ]
                )
                @ np.array(  # somehow needed to have the camera point at the right direction
                    [
                        [1, 0, 0],
                        [0, -1, 0],
                        [0, 0, -1],
                    ]
                )
            )

            rr.log_transform3d(entity, rr.TranslationAndMat3(translation=calib_camera.position, matrix=rot))

            assert calib_sensor.internals.type == "perspective"

            focal_length = calib_sensor.internals.focal_length_px
            u_center = calib_sensor.internals.principal_point_px[0]
            v_center = calib_sensor.internals.principal_point_px[1]
            intrinsics = np.array(
                (
                    (focal_length, 0, u_center),
                    (0, focal_length, v_center),
                    (0, 0, 1),
                )
            )

            # TODO(ab): this is probably affected by https://github.com/rerun-io/rerun/issues/2244. The code will be
            # cleaner (and less buggy) once that is fixed.
            rr.log_pinhole(
                entity + "/image",
                child_from_parent=intrinsics,
                width=sensor.image_size_px[0],
                height=sensor.image_size_px[1],
            )
            rr.log_image_file(entity + "/image/rgb", img_path=self.path.parent / camera.uri)


def main() -> None:
    logging.getLogger().addHandler(logging.StreamHandler())
    logging.getLogger().setLevel("INFO")

    parser = argparse.ArgumentParser(description="Uses the MediaPipe Face Detection to track a human pose in video.")
    parser.add_argument(
        "--dataset",
        choices=DATASETS.keys(),
        default="olympic",
        help="Run on a demo image automatically downloaded",
    )

    rr.script_add_args(parser)

    args, unknown = parser.parse_known_args()
    for arg in unknown:
        logging.warning(f"unknown arg: {arg}")

    # load the data set
    project = OPFProject.from_dataset(args.dataset)

    # display everything in Rerun
    rr.script_setup(args, "open_photogrammetry_format")
    project.log_point_cloud()
    project.log_cameras_as_frames()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
