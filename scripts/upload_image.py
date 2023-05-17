#!/usr/bin/env python3

"""
Upload an image to Google Cloud.

To use this:
- Install the Google Cloud CLI (https://cloud.google.com/storage/docs/gsutil_install)
- Set up credentials (https://cloud.google.com/storage/docs/gsutil_install#authenticate)
"""

import argparse
import hashlib
import os
import subprocess


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
    parser.add_argument("file", type=str, help="Path to the image.")
    args = parser.parse_args()

    object_name = content_hash(args.file) + f"_{os.path.basename(args.file)}"

    subprocess.check_output(["gsutil", "cp", args.file, f"gs://rerun-static-img/{object_name}"])
    print(f"https://static.rerun.io/{object_name}")


if __name__ == "__main__":
    main()
