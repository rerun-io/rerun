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
from tqdm import tqdm

import rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL: Final = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"


def extract_voxel_data(
    dicom_files: Iterable[Path],
) -> Tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
    slices = [dicom.read_file(f) for f in dicom_files]  # type: ignore[misc]
    try:
        voxel_ndarray, ijk_to_xyz = dicom_numpy.combine_slices(slices)
    except dicom_numpy.DicomImportException:
        raise  # invalid DICOM data

    return voxel_ndarray, ijk_to_xyz


def list_dicom_files(dir: Path) -> Iterable[Path]:
    for path, _, files in os.walk(dir):
        for f in files:
            if f.endswith(".dcm"):
                yield Path(path) / f


def read_and_log_dicom_dataset(dicom_files: Iterable[Path]) -> None:
    voxels_volume, _ = extract_voxel_data(dicom_files)

    # the data is i16, but in range [0, 536].
    voxels_volume_u16: npt.NDArray[np.uint16] = np.require(voxels_volume, np.uint16)

    rerun.log_tensor(
        "tensor",
        voxels_volume_u16,
        names=["right", "back", "up"],
    )


def ensure_dataset_downloaded() -> Iterable[Path]:
    dicom_files = [p for p in list_dicom_files(DATASET_DIR)]
    if dicom_files:
        return dicom_files
    print(f"downloading dataset…")
    os.makedirs(DATASET_DIR.absolute(), exist_ok=True)
    resp = requests.get(DATASET_URL, stream=True)
    resp_size = int(resp.headers.get('content-length', 0))
    chunk_size = 1024 # 1 Kibibyte
    fname = os.path.join("/tmp", "dicom.zip")

    with open(fname, "wb") as f:
        for chunk in tqdm(
            resp.iter_content(chunk_size=chunk_size),
            desc=fname, miniters=1,
            total=resp_size, unit='KiB', unit_divisor=1024,
            unit_scale=True):

            f.write(chunk)

    z = zipfile.ZipFile(fname)
    z.extractall(DATASET_DIR.absolute())

    return list_dicom_files(DATASET_DIR)


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
    args = parser.parse_args()

    rerun.init("dicom")

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    dicom_files = ensure_dataset_downloaded()
    read_and_log_dicom_dataset(dicom_files)

    if args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
