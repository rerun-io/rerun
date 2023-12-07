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
from typing import Any

from google.cloud import storage
from jinja2 import Template


def check_expected_wheels(wheels: list[str]) -> None:
    missing = set(["windows", "macos_intel", "macos_arm", "linux"])

    for wheel in wheels:
        if "win_amd64" in wheel:
            missing.remove("windows")
        if "macosx" in wheel and "x86_64" in wheel:
            missing.remove("macos_intel")
        if "macosx" in wheel and "arm64" in wheel:
            missing.remove("macos_arm")
        if "manylinux" in wheel and "x86_64" in wheel:
            missing.remove("linux")

    if len(missing) != 0:
        raise Exception(f"missing built wheels for the following platforms: {', '.join(missing)}")


def generate_pip_index(title: str, dir: str, upload: bool, check: bool) -> None:
    # Initialize the GCS clients
    gcs_client = storage.Client()

    # Prepare the found_builds list
    found_builds = []
    wheels_bucket = gcs_client.bucket("rerun-builds")

    print(f'Checking path: "gs://rerun-builds/{dir}"...')

    found: dict[str, Any] = {}

    # Get the wheel files for the commit
    wheel_blobs = list(wheels_bucket.list_blobs(prefix=dir))
    wheels = [blob.name.split("/")[-1] for blob in wheel_blobs if blob.name.endswith(".whl")]

    if check:
        check_expected_wheels(wheels)

    if len(wheels) > 0:
        print(f"Found wheels: {wheels}")
        found["wheels"] = wheels
        found["title"] = title
        found_builds.append(found)

    template_path = os.path.join(os.path.dirname(os.path.relpath(__file__)), "templates/pip_index.html")

    # Render the Jinja template with the found_builds variable
    with open(template_path) as f:
        template = Template(f.read())

    buffer = io.BytesIO(template.render(found_builds=found_builds).encode("utf-8"))
    buffer.seek(0)

    if upload:
        upload_blob = wheels_bucket.blob(f"{dir}/index.html")
        print(f"Uploading results to {upload_blob.name}")
        upload_blob.upload_from_file(buffer, content_type="text/html")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a minimal pip index")
    parser.add_argument("--title", required=True, help="Index title")
    parser.add_argument("--dir", required=True, help="GCS directory to search in the given bucket")
    parser.add_argument("--upload", action="store_true", help="Upload the index to GCS")
    parser.add_argument(
        "--check", action="store_true", help="Check if all required builds are present in the given bucket directory"
    )
    args = parser.parse_args()

    generate_pip_index(args.title, args.dir, args.upload, args.check)


if __name__ == "__main__":
    main()
