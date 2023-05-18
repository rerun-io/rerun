#!/usr/bin/env python3

"""
Upload an image to Google Cloud.

Requires the following packages:
  pip install google-cloud-storage # NOLINT

Before running, you have to authenticate via the Google Cloud CLI:
- Install it (https://cloud.google.com/storage/docs/gsutil_install)
- Set up credentials (https://cloud.google.com/storage/docs/gsutil_install#authenticate)
"""

import argparse
import hashlib
import mimetypes

from google.cloud import storage


def content_hash(path: str) -> str:
    h = hashlib.sha1()
    b = bytearray(128 * 1024)
    mv = memoryview(b)
    with open(path, "rb", buffering=0) as f:
        while n := f.readinto(mv):
            h.update(mv[:n])
    return h.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description="Upload an image.")
    parser.add_argument("path", type=str, help="Path to the image.")
    args = parser.parse_args()

    hash = content_hash(args.path)
    object_name = f"{hash}_{args.path}"

    gcs = storage.Client()
    bucket = gcs.bucket("rerun-static-img")
    destination = bucket.blob(object_name)
    destination.content_type, destination.content_encoding = mimetypes.guess_type(args.path)
    with open(args.path, "rb") as f:
        destination.upload_from_file(f)

    print(f"https://static.rerun.io/{object_name}")


if __name__ == "__main__":
    main()
