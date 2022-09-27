#!/usr/bin/env pythong3

import argparse
from typing import Final, Iterable
import pydicom as dicom
import dicom_numpy
import numpy as np
import os
from pathlib import Path
import numpy.typing as npt

import rerun_sdk as rerun

DATA_FILES: Final = Path(os.path.dirname(__file__)).joinpath("data").glob("*.dcm")

# extract voxel data
def extract_voxel_data(dicom_files: Iterable[Path]) -> npt.NDArray[np.int16]:
    datasets = [dicom.read_file(f) for f in dicom_files]
    try:
        voxel_ndarray, ijk_to_xyz = dicom_numpy.combine_slices(datasets)
    except dicom_numpy.DicomImportException as e:
    # invalid DICOM data
        raise
    return voxel_ndarray

def log_dicom_data():
    data = extract_voxel_data(DATA_FILES)
    print(data.dtype, data.shape)

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

    log_dicom_data()


if __name__ == '__main__':
    main()
