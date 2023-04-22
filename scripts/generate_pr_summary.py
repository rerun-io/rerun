"""
Script to generate a PR summary page.

This script combines the GitHub and google cloud storage APIs
to find and link to the builds associated with a given PR.

This is expected to be run by the `reusable_pr_summary.yml` GitHub workflow.

Requires the following packages:
  pip install google-cloud-storage Jinja2 PyGithub # NOLINT
"""

import os
from typing import Any, Dict

from github import Github  # NOLINT
from google.cloud import storage
from jinja2 import Template

GITHUB_TOKEN = os.environ["GITHUB_TOKEN"]
GITHUB_REPOSITORY = os.environ["GITHUB_REPOSITORY"]
PR_NUMBER = int(os.environ["PR_NUMBER"])


# Initialize the GitHub and GCS clients
gh = Github(GITHUB_TOKEN)  # NOLINT
gcs_client = storage.Client()

# Get the list of commits associated with the PR
repo = gh.get_repo(GITHUB_REPOSITORY)
pull = repo.get_pull(PR_NUMBER)
all_commits = [commit.sha for commit in pull.get_commits()]
all_commits.reverse()

# Prepare the found_builds list
found_builds = []
viewer_bucket = gcs_client.bucket("rerun-web-viewer")
wheels_bucket = gcs_client.bucket("rerun-builds")

for commit in all_commits:
    commit_short = commit[:7]
    print("Checking commit: {}...".format(commit_short))

    found: Dict[str, Any] = {}

    # Check if there is a hosted app for the current commit
    commit_blob = viewer_bucket.blob(f"commit/{commit_short}/index.html")
    if commit_blob.exists():
        print("Found web assets commit: {}".format(commit_short))
        found["hosted_app"] = f"https://app.rerun.io/commit/{commit_short}"

    # Get the wheel files for the commit
    wheel_blobs = list(wheels_bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
    wheels = [f"https://storage.googleapis.com/{blob.bucket.name}/{blob.name}" for blob in wheel_blobs]
    if wheels:
        print("Found wheels for commit: {}".format(commit_short))
        found["wheels"] = wheels

    if found:
        found["commit"] = commit_short
        found_builds.append(found)

# Render the Jinja template with the found_builds variable
with open("templates/pr_results_summary.html") as f:
    template = Template(f.read())

with open("build_summary.html", "w") as f:
    f.write(template.render(found_builds=found_builds, pr_number=PR_NUMBER))
