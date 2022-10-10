#!/usr/bin/env python3
"""Example using a dicom MRI scan.

Setup:
``` sh
python3 examples/dicom/download_dataset.py
```

Run:
``` sh
python3 examples/dicom/main.py
```
"""

import argparse
import io
import os
import zipfile
from pathlib import Path
from typing import Final, Iterable, Tuple

import dicom_numpy
import numpy as np
import numpy.typing as npt
import pydicom as dicom
import requests
import rerun_sdk as rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL: Final = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"


def extract_voxel_data(
    dicom_files: Iterable[Path],
) -> Tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
    slices = [dicom.read_file(f) for f in dicom_files]
    try:
        voxel_ndarray, ijk_to_xyz = dicom_numpy.combine_slices(slices)
    except dicom_numpy.DicomImportException as e:
        raise  # invalid DICOM data

    return voxel_ndarray, ijk_to_xyz


def list_dicom_files(dir: Path) -> Iterable[Path]:
    for path, _, files in os.walk(dir):
        for f in files:
            if f.endswith(".dcm"):
                yield Path(path) / f


def read_and_log_dicom_dataset() -> None:
    dicom_files = list_dicom_files(DATASET_DIR)
    voxels_volume, _ = extract_voxel_data(dicom_files)

    # the data is i16, but in range [0, 536].
    voxels_volume_u16: npt.NDArray[np.uint16] = np.require(voxels_volume, np.uint16)

    rerun.log_tensor(
        "tensor",
        voxels_volume_u16,
        space="tensor",
        names=["right", "back", "up"],
    )


def download_dataset() -> None:
    print(f"downloading dataset…")
    os.makedirs(DATASET_DIR.absolute(), exist_ok=True)
    resp = requests.get(DATASET_URL, stream=True)
    z = zipfile.ZipFile(io.BytesIO(resp.content))
    z.extractall(DATASET_DIR.absolute())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--connect",
        dest="connect",
        action="store_true",
        help="Connect to an external viewer",
    )
    parser.add_argument("--addr", type=str, default=None, help="Connect to this ip:port")
    parser.add_argument("--save", type=str, default=None, help="Save data to a .rrd file at this path")
    parser.add_argument("--headless", action="store_true", help="Don't show GUI")
    parser.add_argument("--download", action="store_true", help="Download dataset")
    args = parser.parse_args()

    if args.download:
        download_dataset()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    read_and_log_dicom_dataset()

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()

    rerun.show()
