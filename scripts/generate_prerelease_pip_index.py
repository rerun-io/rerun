"""
Script to generate a minimal pip index.

This script use the google cloud storage APIs to find and link to the builds
associated with a given commit.

This is expected to be run by the `reusable_pip_index.yml` GitHub workflow.

Requires the following packages:
  pip install google-cloud-storage Jinja2 PyGithub # NOLINT
"""

import io
import os
from typing import Any, Dict

from google.cloud import storage
from jinja2 import Template

GITHUB_SHA = os.environ["GITHUB_SHA"]

# Initialize the GCS clients
gcs_client = storage.Client()

# Prepare the found_builds list
found_builds = []
wheels_bucket = gcs_client.bucket("rerun-builds")

commit = GITHUB_SHA[:7]

commit_short = commit[:7]
print("Checking commit: {}...".format(commit_short))

found: Dict[str, Any] = {}

# Get the wheel files for the commit
wheel_blobs = list(wheels_bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
wheels = [blob.name.split("/")[-1] for blob in wheel_blobs if blob.name.endswith(".whl")]
if wheels:
    print("Found wheels for commit: {}: {}".format(commit_short, wheels))
    found["wheels"] = wheels

if found:
    found["commit"] = commit_short
    found_builds.append(found)

# Render the Jinja template with the found_builds variable
with open("templates/pypi_index.html") as f:
    template = Template(f.read())

buffer = io.BytesIO(template.render(found_builds=found_builds).encode("utf-8"))
buffer.seek(0)

upload_blob = wheels_bucket.blob(f"commit/{commit}/wheels/index.html")
upload_blob.upload_from_file(buffer, content_type="text/html")
