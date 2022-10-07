#!/usr/bin/env python3

import io
import os
import zipfile
from pathlib import Path
from typing import Final

import requests

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"


def download_dataset():
    url = "https://storage.googleapis.com/rerun-example-datasets/dicom.zip"

    os.makedirs(DATASET_DIR.absolute(), exist_ok=True)

    print(f"downloading dataset…")
    resp = requests.get(url, stream=True)
    z = zipfile.ZipFile(io.BytesIO(resp.content))
    z.extractall(DATASET_DIR.absolute())

if __name__ == '__main__':
    download_dataset()
