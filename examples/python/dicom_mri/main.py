#!/usr/bin/env python3
"""
Example using MRI scan data in the DICOM format.

Run:
``` sh
python3 examples/python/dicom/main.py
```
"""
from __future__ import annotations

import argparse
import io
import os
import zipfile
from pathlib import Path
from typing import Final, Iterable

import dicom_numpy
import numpy as np
import numpy.typing as npt
import pydicom as dicom
import requests
import rerun as rr  # pip install rerun-sdk

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL: Final = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"

DESCRIPTION = """
# Dicom MRI
This example visualizes an MRI scan using Rerun.

## How it was made
The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/dicom_mri/main.py).

The visualization of the data consists of just the following line
```python
rr.log("tensor", rr.Tensor(voxels_volume_u16, dim_names=["right", "back", "up"]))
```

`voxels_volume_u16` is a `numpy.array` of shape `(512, 512, 512)` containing volumetric MRI intensities. We can
visualize such information in Rerun by logging the `numpy.array` as an
[rr.Tensor archetype](https://www.rerun.io/docs/reference/types/archetypes/tensor). Here the tensor is logged to
the [tensor entity](recording://tensor), however any other name for the entity could have been chosen.

In the Rerun viewer you can inspect the data in detail. The `dim_names` provided in the above call to `rr.log` help to
give semantic meaning to each axis. After selecting the tensor view, you can adjust various settings in the Blueprint
settings on the right-hand side. For example, you can adjust the color map, the brightness, which dimensions to show as
an image and which to select from, and more.
"""


def extract_voxel_data(
    dicom_files: Iterable[Path],
) -> tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
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
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    voxels_volume, _ = extract_voxel_data(dicom_files)

    # the data is i16, but in range [0, 536].
    voxels_volume_u16: npt.NDArray[np.uint16] = np.require(voxels_volume, np.uint16)

    rr.log("tensor", rr.Tensor(voxels_volume_u16, dim_names=["right", "back", "up"]))


def ensure_dataset_downloaded() -> Iterable[Path]:
    dicom_files = list(list_dicom_files(DATASET_DIR))
    if dicom_files:
        return dicom_files
    print("downloading dataset…")
    os.makedirs(DATASET_DIR.absolute(), exist_ok=True)
    resp = requests.get(DATASET_URL, stream=True)
    z = zipfile.ZipFile(io.BytesIO(resp.content))
    z.extractall(DATASET_DIR.absolute())

    return list_dicom_files(DATASET_DIR)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Example using MRI scan data in the DICOM format.")
    rr.script_add_args(parser)
    args = parser.parse_args()
    rr.script_setup(args, "rerun_example_dicom_mri")
    dicom_files = ensure_dataset_downloaded()
    read_and_log_dicom_dataset(dicom_files)
    rr.script_teardown(args)
