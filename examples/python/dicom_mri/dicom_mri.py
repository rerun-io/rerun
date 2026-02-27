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

if TYPE_CHECKING:
    from collections.abc import Iterable

DESCRIPTION = """
# Dicom MRI
This example visualizes an MRI scan using Rerun.

The visualization includes both a standard tensor view of the volume data and
an experimental custom shader-based volumetric rendering using raymarching.

The tensor visualization is a single line:
```python
rr.log("tensor", rr.Tensor(voxels_volume_u16, dim_names=["right", "back", "up"]))
```

The volumetric rendering logs a bounding box mesh with a custom WGSL fragment shader
that raymarches through a 3D texture.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/dicom_mri).
"""

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
DATASET_URL: Final = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"


def extract_voxel_data(
    dicom_files: Iterable[Path],
) -> tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
    slices = [dicom.dcmread(f) for f in dicom_files]
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


def make_volume_bbox_mesh(
    shape: tuple[int, ...],
) -> tuple[npt.NDArray[np.float32], npt.NDArray[np.uint32]]:
    """Create a unit cube mesh for volume rendering.

    Returns (positions, indices). The vertex positions span [0,1]^3 in object space,
    which maps directly to volume texture coordinates. The custom shader uses
    position_object (from the vertex shader) to determine volume coordinates.
    """
    d, h, w = float(shape[0]), float(shape[1]), float(shape[2])
    # Normalize so the longest axis is 1.0
    max_dim = max(d, h, w)
    sx, sy, sz = w / max_dim, h / max_dim, d / max_dim

    # 8 corners of the bounding box in [0,1]^3 object space (= volume coordinates)
    positions = np.array(
        [
            [0, 0, 0],
            [sx, 0, 0],
            [sx, sy, 0],
            [0, sy, 0],
            [0, 0, sz],
            [sx, 0, sz],
            [sx, sy, sz],
            [0, sy, sz],
        ],
        dtype=np.float32,
    )

    # 12 triangles (2 per face)
    indices = np.array(
        [
            [0, 2, 1], [0, 3, 2],  # front
            [4, 5, 6], [4, 6, 7],  # back
            [0, 1, 5], [0, 5, 4],  # bottom
            [2, 3, 7], [2, 7, 6],  # top
            [0, 4, 7], [0, 7, 3],  # left
            [1, 2, 6], [1, 6, 5],  # right
        ],
        dtype=np.uint32,
    )

    return positions, indices


def log_volumetric_rendering(voxels_volume: npt.NDArray[np.int16], shape: tuple[int, ...]) -> None:
    """Log volume data with a custom raymarching shader for 3D rendering."""
    example_dir = Path(os.path.dirname(__file__))

    # Load the pre-compiled WGSL shader and parameters
    wgsl_path = example_dir / "volume_raymarch.wgsl"
    params_path = example_dir / "volume_raymarch_params.json"

    if not wgsl_path.exists() or not params_path.exists():
        print("Volumetric shader files not found, skipping 3D volume rendering.")
        return

    wgsl_source = wgsl_path.read_text()
    params_json = params_path.read_text()

    # Create bounding box mesh (positions in [0,1]^3 = volume coordinates)
    positions, indices = make_volume_bbox_mesh(shape)

    # Log the mesh with custom shader
    rr.log(
        "volume/mesh",
        rr.Mesh3D(
            vertex_positions=positions,
            triangle_indices=indices,
            shader_source=wgsl_source,
            shader_parameters=params_json,
        ),
        static=True,
    )

    # Log the volume data as a 3D tensor (to be bound as texture by the shader).
    # Downscale to fit within GPU 3D texture limits (typically 256 per dimension).
    max_dim = 128
    from scipy.ndimage import zoom as ndimage_zoom

    scale_factors = tuple(max_dim / s if s > max_dim else 1.0 for s in shape)
    voxels_small = ndimage_zoom(voxels_volume.astype(np.float32), scale_factors, order=1)
    rr.log("volume/mesh/volume_data", rr.Tensor(voxels_small, dim_names=["depth", "height", "width"]), static=True)

    # Log shader parameters as queryable values
    # The data is i16 in range [0, 536]
    rr.log("volume/mesh/density_scale", rr.Scalars(1.0), static=True)
    rr.log("volume/mesh/value_range", rr.Tensor(np.array([0.0, 536.0], dtype=np.float32)), static=True)


def read_and_log_dicom_dataset(dicom_files: Iterable[Path]) -> None:
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    voxels_volume, _ = extract_voxel_data(dicom_files)

    # the data is i16, but in range [0, 536].
    voxels_volume_u16: npt.NDArray[np.uint16] = np.require(voxels_volume, np.uint16)

    rr.log("tensor", rr.Tensor(voxels_volume_u16, dim_names=["right", "back", "up"]))

    # Experimental: Log volumetric rendering with custom shader
    log_volumetric_rendering(voxels_volume, voxels_volume.shape)


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
    dicom_files = ensure_dataset_downloaded()
    read_and_log_dicom_dataset(dicom_files)
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
