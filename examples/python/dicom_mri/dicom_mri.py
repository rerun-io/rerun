#!/usr/bin/env python3
"""Example using MRI scan data in the DICOM format."""

from __future__ import annotations

import argparse
import io
import os
import zipfile
from pathlib import Path
from typing import TYPE_CHECKING, Final

import dicom_numpy
import numpy as np
import numpy.typing as npt
import pydicom as dicom
import requests

import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb
from rerun.blueprint.encodings import ComponentSourceKind, VisualizerComponentMapping

if TYPE_CHECKING:
    from collections.abc import Iterable

DESCRIPTION = """
# Dicom MRI
This example visualizes an MRI scan using Rerun.

The same logged `Volume3D:values` component is shown as a sliceable tensor and as a raymarched 3D volume:
```python
values = rr.TensorData(array=voxels_volume_f16, dim_names=["up", "back", "right"])
rr.log("volume", rr.Volume3D(values))
```

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/dicom_mri).
"""

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL: Final = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"


def extract_voxel_data(
    dicom_files: Iterable[Path],
) -> tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
    slices = [dicom.read_file(f) for f in dicom_files]  # type: ignore[misc]
    voxel_ndarray, ijk_to_xyz = dicom_numpy.combine_slices(slices)

    return voxel_ndarray, ijk_to_xyz


def list_dicom_files(dir: Path) -> Iterable[Path]:
    for path, _, files in os.walk(dir):
        for f in files:
            if f.endswith(".dcm"):
                yield Path(path) / f


def read_and_log_dicom_dataset(dicom_files: Iterable[Path]) -> None:
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    voxels_volume, ijk_to_xyz = extract_voxel_data(dicom_files)

    # Convert source [i, j, k] axes to Volume3D's [z, y, x] storage order.
    voxels_volume_f16 = voxels_volume.T.astype(np.float16)

    values = rr.TensorData(array=voxels_volume_f16, dim_names=["up", "back", "right"])
    rr.log(
        "volume",
        rr.Transform3D(translation=ijk_to_xyz[:3, 3], mat3x3=ijk_to_xyz[:3, :3]),
        static=True,
    )
    rr.log(
        "volume",
        rr.Volume3D(
            values,
            optical_density=5.0,
        ),
    )


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


def main() -> None:
    parser = argparse.ArgumentParser(description="Example using MRI scan data in the DICOM format.")
    rr.script_add_args(parser)
    args = parser.parse_args()
    rr.script_setup(args, "rerun_example_dicom_mri")
    tensor_visualizer = rr.Tensor.from_fields().visualizer(
        mappings=[
            VisualizerComponentMapping(
                target="Tensor:data",
                source_kind=ComponentSourceKind.SourceComponent,
                source_component="Volume3D:values",
            )
        ]
    )
    rr.send_blueprint(
        rrb.Horizontal(
            rrb.Vertical(
                rrb.TextDocumentView(name="Description", origin="/description"),
                rrb.TensorView(
                    name="MRI slices",
                    overrides={"/volume": tensor_visualizer},
                ),
            ),
            rrb.Spatial3DView(name="MRI volume"),
        )
    )
    dicom_files = ensure_dataset_downloaded()
    read_and_log_dicom_dataset(dicom_files)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
