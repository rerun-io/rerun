# Copied from https://github.com/apple/ARKitScenes/blob/9ec0b99c3cd55e29fc0724e1229e2e6c2909ab45/download_data.py
# Licensing information: https://github.com/apple/ARKitScenes/blob/9ec0b99c3cd55e29fc0724e1229e2e6c2909ab45/LICENSE
from __future__ import annotations

import math
import os
import subprocess
import zipfile
from pathlib import Path
from typing import Final

import pandas as pd

ARkitscense_url = "https://docs-assets.developer.apple.com/ml-research/datasets/arkitscenes/v1"
TRAINING: Final = "Training"
VALIDATION: Final = "Validation"
HIGRES_DEPTH_ASSET_NAME: Final = "highres_depth"
POINT_CLOUDS_FOLDER: Final = "laser_scanner_point_clouds"

AVAILABLE_RECORDINGS: Final = ["48458663", "42444949", "41069046", "41125722", "41125763", "42446167"]
DATASET_DIR: Final = Path(__file__).parent.parent / "dataset"

default_raw_dataset_assets = [
    "mov",
    "annotation",
    "mesh",
    "confidence",
    "highres_depth",
    "lowres_depth",
    "lowres_wide.traj",
    "lowres_wide",
    "lowres_wide_intrinsics",
    "ultrawide",
    "ultrawide_intrinsics",
    "vga_wide",
    "vga_wide_intrinsics",
]

missing_3dod_assets_video_ids = [
    "47334522",
    "47334523",
    "42897421",
    "45261582",
    "47333152",
    "47333155",
    "48458535",
    "48018733",
    "47429677",
    "48458541",
    "42897848",
    "47895482",
    "47333960",
    "47430089",
    "42899148",
    "42897612",
    "42899153",
    "42446164",
    "48018149",
    "47332198",
    "47334515",
    "45663223",
    "45663226",
    "45663227",
]


def raw_files(video_id: str, assets: list[str], metadata: pd.DataFrame) -> list[str]:
    file_names = []
    for asset in assets:
        if HIGRES_DEPTH_ASSET_NAME == asset:
            in_upsampling = metadata.loc[metadata["video_id"] == float(video_id), ["is_in_upsampling"]].iat[0, 0]
            if not in_upsampling:
                print(f"Skipping asset {asset} for video_id {video_id} - Video not in upsampling dataset")
                continue  # highres_depth asset only available for video ids from upsampling dataset

        if asset in [
            "confidence",
            "highres_depth",
            "lowres_depth",
            "lowres_wide",
            "lowres_wide_intrinsics",
            "ultrawide",
            "ultrawide_intrinsics",
            "wide",
            "wide_intrinsics",
            "vga_wide",
            "vga_wide_intrinsics",
        ]:
            file_names.append(asset + ".zip")
        elif asset == "mov":
            file_names.append(f"{video_id}.mov")
        elif asset == "mesh":
            if video_id not in missing_3dod_assets_video_ids:
                file_names.append(f"{video_id}_3dod_mesh.ply")
        elif asset == "annotation":
            if video_id not in missing_3dod_assets_video_ids:
                file_names.append(f"{video_id}_3dod_annotation.json")
        elif asset == "lowres_wide.traj":
            if video_id not in missing_3dod_assets_video_ids:
                file_names.append("lowres_wide.traj")
        else:
            raise Exception(f"No asset = {asset} in raw dataset")
    return file_names


def download_file(url: str, file_name: str, dst: Path) -> bool:
    os.makedirs(dst, exist_ok=True)
    filepath = os.path.join(dst, file_name)

    if not os.path.isfile(filepath):
        command = f"curl {url} -o {file_name}.tmp --fail"
        print(f"Downloading file {filepath}")
        try:
            subprocess.check_call(command, shell=True, cwd=dst)
        except Exception as error:
            print(f"Error downloading {url}, error: {error}")
            return False
        os.rename(filepath + ".tmp", filepath)
    else:
        pass  # skipping download of existing file
    return True


def unzip_file(file_name: str, dst: Path, keep_zip: bool = True) -> bool:
    filepath = os.path.join(dst, file_name)
    print(f"Unzipping zip file {filepath}")
    try:
        with zipfile.ZipFile(filepath, "r") as zip_ref:
            zip_ref.extractall(dst)
    except Exception as error:
        print(f"Error unzipping {filepath}, error: {error}")
        return False
    if not keep_zip:
        os.remove(filepath)
    return True


def download_laser_scanner_point_clouds_for_video(video_id: str, metadata: pd.DataFrame, download_dir: Path) -> None:
    video_metadata = metadata.loc[metadata["video_id"] == float(video_id)]
    visit_id = video_metadata["visit_id"].iat[0]
    has_laser_scanner_point_clouds = video_metadata["has_laser_scanner_point_clouds"].iat[0]

    if not has_laser_scanner_point_clouds:
        print(f"Warning: Laser scanner point clouds for video {video_id} are not available")
        return

    if math.isnan(visit_id) or not visit_id.is_integer():
        print(f"Warning: Downloading laser scanner point clouds for video {video_id} failed - Bad visit id {visit_id}")
        return

    visit_id = int(visit_id)  # Expecting an 8 digit integer
    laser_scanner_point_clouds_ids = laser_scanner_point_clouds_for_visit_id(visit_id, download_dir)

    for point_cloud_id in laser_scanner_point_clouds_ids:
        download_laser_scanner_point_clouds(point_cloud_id, visit_id, download_dir)


def laser_scanner_point_clouds_for_visit_id(visit_id: int, download_dir: Path) -> list[str]:
    point_cloud_to_visit_id_mapping_filename = "laser_scanner_point_clouds_mapping.csv"
    if not os.path.exists(point_cloud_to_visit_id_mapping_filename):
        point_cloud_to_visit_id_mapping_url = (
            f"{ARkitscense_url}/raw/laser_scanner_point_clouds/{point_cloud_to_visit_id_mapping_filename}"
        )
        if not download_file(
            point_cloud_to_visit_id_mapping_url,
            point_cloud_to_visit_id_mapping_filename,
            download_dir,
        ):
            print(
                f"Error downloading point cloud for visit_id {visit_id} at location "
                f"{point_cloud_to_visit_id_mapping_url}",
            )
            return []

    point_cloud_to_visit_id_mapping_filepath = os.path.join(download_dir, point_cloud_to_visit_id_mapping_filename)
    point_cloud_to_visit_id_mapping = pd.read_csv(point_cloud_to_visit_id_mapping_filepath)
    point_cloud_ids = point_cloud_to_visit_id_mapping.loc[
        point_cloud_to_visit_id_mapping["visit_id"] == visit_id,
        ["laser_scanner_point_clouds_id"],
    ]
    point_cloud_ids_list = [scan_id[0] for scan_id in point_cloud_ids.values]

    return point_cloud_ids_list


def download_laser_scanner_point_clouds(laser_scanner_point_cloud_id: str, visit_id: int, download_dir: Path) -> None:
    laser_scanner_point_clouds_folder_path = download_dir / POINT_CLOUDS_FOLDER / str(visit_id)
    os.makedirs(laser_scanner_point_clouds_folder_path, exist_ok=True)

    for extension in [".ply", "_pose.txt"]:
        filename = f"{laser_scanner_point_cloud_id}{extension}"
        filepath = os.path.join(laser_scanner_point_clouds_folder_path, filename)
        if os.path.exists(filepath):
            return
        file_url = f"{ARkitscense_url}/raw/laser_scanner_point_clouds/{visit_id}/{filename}"
        download_file(file_url, filename, laser_scanner_point_clouds_folder_path)


def get_metadata(dataset: str, download_dir: Path) -> pd.DataFrame | None:
    filename = "metadata.csv"
    url = f"{ARkitscense_url}/threedod/{filename}" if "3dod" == dataset else f"{ARkitscense_url}/{dataset}/{filename}"
    dst_folder = download_dir / dataset
    dst_file = dst_folder / filename

    if not download_file(url, filename, dst_folder):
        return None

    metadata = pd.read_csv(dst_file)
    return metadata


def download_data(
    dataset: str,
    video_ids: list[str],
    dataset_splits: list[str],
    download_dir: Path,
    keep_zip: bool,
    raw_dataset_assets: list[str] | None = None,
    should_download_laser_scanner_point_cloud: bool = False,
) -> None:
    """
    Downloads data from the specified dataset and video IDs to the given download directory.

    Args:
    ----
    dataset: the name of the dataset to download from (raw, 3dod, or upsampling)
    video_ids: the list of video IDs to download data for
    dataset_splits: the list of splits for each video ID (train, validation, or test)
    download_dir: the directory to download data to
    keep_zip: whether to keep the downloaded zip files after extracting them
    raw_dataset_assets: a list of asset types to download from the raw dataset, if dataset is "raw"
    should_download_laser_scanner_point_cloud: whether to download the laser scanner point cloud data, if available

    Returns: None

    """
    metadata = get_metadata(dataset, download_dir)
    if metadata is None:
        print(f"Error retrieving metadata for dataset {dataset}")
        return

    for video_id in sorted(set(video_ids)):
        split = dataset_splits[video_ids.index(video_id)]
        dst_dir = download_dir / dataset / split
        if dataset == "raw":
            url_prefix = ""
            file_names = []
            if not raw_dataset_assets:
                print(f"Warning: No raw assets given for video id {video_id}")
            else:
                dst_dir = dst_dir / str(video_id)
                url_prefix = f"{ARkitscense_url}/raw/{split}/{video_id}" + "/{}"
                file_names = raw_files(video_id, raw_dataset_assets, metadata)
        elif dataset == "3dod":
            url_prefix = f"{ARkitscense_url}/threedod/{split}" + "/{}"
            file_names = [
                f"{video_id}.zip",
            ]
        elif dataset == "upsampling":
            url_prefix = f"{ARkitscense_url}/upsampling/{split}" + "/{}"
            file_names = [
                f"{video_id}.zip",
            ]
        else:
            raise Exception(f"No such dataset = {dataset}")

        if should_download_laser_scanner_point_cloud and dataset == "raw":
            # Point clouds only available for the raw dataset
            download_laser_scanner_point_clouds_for_video(video_id, metadata, download_dir)

        for file_name in file_names:
            dst_path = os.path.join(dst_dir, file_name)
            url = url_prefix.format(file_name)

            if not file_name.endswith(".zip") or not os.path.isdir(dst_path[: -len(".zip")]):
                download_file(url, dst_path, dst_dir)
            else:
                pass  # skipping download of existing zip file
            if file_name.endswith(".zip") and os.path.isfile(dst_path):
                unzip_file(file_name, dst_dir, keep_zip)


def ensure_recording_downloaded(video_id: str, include_highres: bool) -> Path:
    """Only downloads from validation set."""
    data_path = DATASET_DIR / "raw" / "Validation" / video_id
    assets_to_download = [
        "lowres_wide",
        "lowres_depth",
        "lowres_wide_intrinsics",
        "lowres_wide.traj",
        "annotation",
        "mesh",
    ]
    if include_highres:
        assets_to_download.extend(["highres_depth", "wide", "wide_intrinsics"])
    download_data(
        dataset="raw",
        video_ids=[video_id],
        dataset_splits=[VALIDATION],
        download_dir=DATASET_DIR,
        keep_zip=False,
        raw_dataset_assets=assets_to_download,
        should_download_laser_scanner_point_cloud=False,
    )
    return data_path


def ensure_recording_available(video_id: str, include_highres: bool) -> Path:
    """
    Returns the path to the recording for a given video_id.

    Args:
    ----
    video_id (str):
        Identifier for the recording.
    include_highres (bool):
        Whether to include the high resolution recording.

    Returns
    -------
    Path: Path object representing the path to the recording.

    Raises
    ------
    AssertionError:
        If the recording path does not exist.

    """
    recording_path = ensure_recording_downloaded(video_id, include_highres)
    assert recording_path.exists(), f"Recording path {recording_path} does not exist."
    return recording_path  # Return the path to the recording
