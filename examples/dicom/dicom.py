#!/usr/bin/env pythong3

import argparse
from typing import Final, Iterable, Tuple
import pydicom as dicom
import dicom_numpy   # type: ignore
import numpy as np
import os
from pathlib import Path
import numpy.typing as npt

import rerun_sdk as rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"

# extract voxel data
def extract_voxel_data(dicom_files: Iterable[Path]) -> Tuple[npt.NDArray[np.int16], npt.NDArray[np.float32]]:
    slices = [dicom.read_file(f) for f in dicom_files]
    try:
        voxel_ndarray, ijk_to_xyz = dicom_numpy.combine_slices(slices, rescale=True)
    except dicom_numpy.DicomImportException as e:
    # invalid DICOM data
        raise

    return voxel_ndarray, ijk_to_xyz


def list_viacom_files(dir: Path) -> Iterable[Path]:
    for path, _, files in os.walk(dir):
        for f in files:
            if f.endswith(".dcm"):
                yield Path(path) / f


def main() -> None:
    parser = argparse.ArgumentParser(
        description='Logs rich data using the Rerun SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    args = parser.parse_args()

    dicom_files = list_viacom_files(DATASET_DIR)
    voxels_volume, _ = extract_voxel_data(dicom_files)

    for slice_idx in range(voxels_volume.shape[-1]):
        rerun.set_time_sequence("slice_idx", slice_idx)
        rerun.log_image("vicom/slice", voxels_volume[:, :, slice_idx], space="mri/xy")

    rerun.show()


if __name__ == '__main__':
    main()
