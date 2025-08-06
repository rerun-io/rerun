#!/usr/bin/env python3

"""
Upload an .rrd to Google Cloud.

Installation
------------

Requires the following packages:
  pip install google-cloud-storage

Before running, you have to authenticate via the Google Cloud CLI:
- Install it (https://cloud.google.com/storage/docs/gsutil_install)
- Set up credentials (https://cloud.google.com/storage/docs/gsutil_install#authenticate)

If you get this error:

    File "…/site-packages/cryptography/hazmat/primitives/asymmetric/utils.py", line 6, in <module>
        from cryptography.hazmat.bindings._rust import asn1
    pyo3_runtime.PanicException: Python API call failed

Then run `python3 -m pip install cryptography==38.0.4`
(https://levelup.gitconnected.com/fix-attributeerror-module-lib-has-no-attribute-openssl-521a35d83769)

Usage
-----

Use the script:

    python3 scripts/upload_rrd.py --help

or the pixi command:

    pixi run upload-rrd --help
"""

from __future__ import annotations

import argparse
import hashlib
import logging
import re
import sys
from io import BytesIO
from pathlib import Path

from google.cloud import storage


class Uploader:
    def __init__(self) -> None:
        gcs = storage.Client("rerun-open")
        self.bucket = gcs.bucket("rerun-rrd")

    def upload_data(
        self,
        data: bytes,
        gcs_path: str,
        content_type: str | None = None,
        content_encoding: str | None = None,
    ) -> None:
        """
        Low-level upload of data.

        Parameters
        ----------
        data:
            The data to upload.
        gcs_path:
            The path of the object.
        content_type:
            The content type of the object.
        content_encoding:
            The content encoding of the object.

        """

        logging.info(f"Uploading {gcs_path} (size: {len(data)}, type: {content_type}, encoding: {content_encoding})")
        destination = self.bucket.blob(gcs_path)
        destination.content_type = content_type
        destination.content_encoding = content_encoding

        if destination.exists():
            logging.warning(f"blob {gcs_path} already exists in GCS, skipping upload")
            return

        stream = BytesIO(data)
        destination.upload_from_file(stream)


def data_hash(data: bytes) -> str:
    """Compute a sha1 hash digest of some data."""
    return hashlib.sha1(data).hexdigest()


DESCRIPTION = """Upload an .rrd to static.rerun.io.

    pixi run upload-rrd --version 0.15.0 path/to/recording.rrd

    The version is used for two things:
    A) used as a folder name in the GCS bucket.
    B) used to generate a link to the correct version of the Rerun web viewer.
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=DESCRIPTION, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument("path", type=str, help="Recording .rrd to upload")
    parser.add_argument(
        "--name",
        type=str,
        required=False,
        help="Name of the recording. If not supplied, the file name is used.",
    )
    parser.add_argument("--version", type=str, required=True, help="The Rerun version, e.g. '0.15.0'.")
    parser.add_argument("--debug", action="store_true", help="Enable debug logging.")
    args = parser.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.DEBUG)
    else:
        logging.basicConfig(level=logging.INFO)

    try:
        if not args.path.endswith(".rrd"):
            raise RuntimeError("File path expected to have .rrd extension")

        file_path = Path(args.path)
        name = args.name or file_path.stem
        version = args.version

        # Check if user put `v0.15.0` instead of `0.15.0`:
        if m := re.match(r"v(\d+\.\d+\..*)", version):
            version = m.group(1)
            raise RuntimeError(f"Version should be in the format '{version}', without a leading 'v'")

        file_data = file_path.read_bytes()
        digest = data_hash(file_data)
        gcp_path = f"{version}/{name}_{digest}.rrd"

        uploader = Uploader()
        uploader.upload_data(file_data, gcp_path, content_type="application/octet-stream")

        recording_url = f"https://static.rerun.io/rrd/{gcp_path}"
        print(f"Recording at: {recording_url}")
        print(f"View it at:   https://rerun.io/viewer/version/{version}/?url={recording_url}")

    except RuntimeError as e:
        print(f"Error: {e.args[0]}", file=sys.stderr)
        return


if __name__ == "__main__":
    main()
