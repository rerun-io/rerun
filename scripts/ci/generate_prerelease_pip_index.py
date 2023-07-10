"""
Script to generate a minimal pip index.

This script use the google cloud storage APIs to find and link to the builds
associated with a given commit.

This is expected to be run by the `reusable_pip_index.yml` GitHub workflow.

Requires the following packages:
  pip install google-cloud-storage Jinja2 PyGithub
"""
from __future__ import annotations

import argparse
import io
import os
from typing import Any

from google.cloud import storage
from jinja2 import Template


def generate_pip_index(commit: str, upload: bool) -> None:
    # Initialize the GCS clients
    gcs_client = storage.Client()

    # Prepare the found_builds list
    found_builds = []
    wheels_bucket = gcs_client.bucket("rerun-builds")

    commit_short = commit[:7]
    print(f"Checking commit: {commit_short}...")

    found: dict[str, Any] = {}

    # Get the wheel files for the commit
    wheel_blobs = list(wheels_bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
    wheels = [blob.name.split("/")[-1] for blob in wheel_blobs if blob.name.endswith(".whl")]
    if wheels:
        print(f"Found wheels for commit: {commit_short}: {wheels}")
        found["wheels"] = wheels

    if found:
        found["commit"] = commit_short
        found_builds.append(found)

    template_path = os.path.join(os.path.dirname(os.path.relpath(__file__)), "templates/pip_index.html")

    # Render the Jinja template with the found_builds variable
    with open(template_path) as f:
        template = Template(f.read())

    buffer = io.BytesIO(template.render(found_builds=found_builds).encode("utf-8"))
    buffer.seek(0)

    if upload:
        upload_blob = wheels_bucket.blob(f"commit/{commit_short}/wheels/index.html")
        print(f"Uploading results to {upload_blob.name}")
        upload_blob.upload_from_file(buffer, content_type="text/html")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a minimal pip index")
    parser.add_argument("--commit", required=True, help="Commit SHA")
    parser.add_argument("--upload", action="store_true", help="Upload the index to GCS")
    args = parser.parse_args()

    generate_pip_index(args.commit, args.upload)


if __name__ == "__main__":
    main()
