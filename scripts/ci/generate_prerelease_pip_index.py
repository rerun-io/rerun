#!/usr/bin/env python3

"""
Script to generate a minimal pip index.

This script use the google cloud storage APIs to find and link to the builds
associated with a given commit.

This is expected to be run by the `reusable_pip_index.yml` GitHub workflow.

Requires the following packages:
  pip install google-cloud-storage Jinja2
"""

from __future__ import annotations

import argparse
import io
import os
import time
from typing import Any

import wheel_utils
from google.cloud import storage
from jinja2 import Template


def generate_pip_index(title: str, dir: str, upload: bool, check: bool) -> None:
    overall_start = time.time()

    # Initialize the GCS clients
    t0 = time.time()
    gcs_client = storage.Client()
    print(f"GCS client initialized in {time.time() - t0:.2f}s")

    # Prepare the found_builds list
    found_builds = []
    wheels_bucket = gcs_client.bucket("rerun-builds")

    print(f'Listing blobs at "gs://rerun-builds/{dir}"…')

    found: dict[str, Any] = {}

    # Get the wheel files for the commit
    t0 = time.time()
    wheel_blobs = list(wheels_bucket.list_blobs(prefix=dir))
    list_duration = time.time() - t0
    wheels = [blob.name.split("/")[-1] for blob in wheel_blobs if blob.name.endswith(".whl")]
    non_wheel_blobs = [blob.name.split("/")[-1] for blob in wheel_blobs if not blob.name.endswith(".whl")]

    print(
        f"Listed {len(wheel_blobs)} blob(s) in {list_duration:.2f}s ({len(wheels)} wheel(s), {len(non_wheel_blobs)} other)"
    )
    if non_wheel_blobs:
        print(f"Non-wheel blobs: {non_wheel_blobs}")

    if check:
        wheel_utils.check_expected_wheels(wheels)

    if len(wheels) > 0:
        for w in wheels:
            print(f"  {w}")
        found["wheels"] = wheels
        found["title"] = title
        found_builds.append(found)
    else:
        print("WARNING: No wheels found!")

    template_path = os.path.join(os.path.dirname(os.path.relpath(__file__)), "templates/pip_index.html")

    # Render the Jinja template with the found_builds variable
    with open(template_path, encoding="utf8") as f:
        template = Template(f.read())

    buffer = io.BytesIO(template.render(found_builds=found_builds).encode("utf-8"))
    index_size = buffer.tell()
    buffer.seek(0)
    print(f"Generated index.html ({index_size} bytes)")

    if upload:
        upload_blob = wheels_bucket.blob(f"{dir}/index.html")
        # Set no-cache to avoid CDN caching issues when rebuilding for the same commit
        upload_blob.cache_control = "no-cache, max-age=0"
        print(f"Uploading to {upload_blob.name}…")
        t0 = time.time()
        upload_blob.upload_from_file(buffer, content_type="text/html")
        print(f"Upload completed in {time.time() - t0:.2f}s")
    else:
        print("Skipping upload (--upload not set)")

    print(f"Total time: {time.time() - overall_start:.2f}s")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a minimal pip index")
    parser.add_argument("--title", required=True, help="Index title")
    parser.add_argument("--dir", required=True, help="GCS directory to search in rerun-builds")
    parser.add_argument("--upload", action="store_true", help="Upload the index to GCS")
    parser.add_argument("--check", action="store_true", help="Check if all required builds are present")
    args = parser.parse_args()

    generate_pip_index(args.title, args.dir, args.upload, args.check)


if __name__ == "__main__":
    main()
