"""
Script to generate a PR summary page.

This script combines the GitHub and google cloud storage APIs
to find and link to the builds associated with a given PR.

This is expected to be run by the `reusable_pr_summary.yml` GitHub workflow.

Requires the following packages:
  pip install google-cloud-storage Jinja2 PyGithub # NOLINT
"""

import argparse
import io
import os
from typing import Any, Dict

from github import Github  # NOLINT
from google.cloud import storage
from jinja2 import Template


def generate_pr_summary(github_token: str, github_repository: str, pr_number: int, upload: bool) -> None:
    # Initialize the GitHub and GCS clients
    gh = Github(github_token)  # NOLINT
    gcs_client = storage.Client()

    # Get the list of commits associated with the PR
    repo = gh.get_repo(github_repository)
    pull = repo.get_pull(pr_number)
    all_commits = [commit.sha for commit in pull.get_commits()]
    all_commits.reverse()

    # Prepare the found_builds list
    found_builds = []
    viewer_bucket = gcs_client.bucket("rerun-web-viewer")
    builds_bucket = gcs_client.bucket("rerun-builds")

    for commit in all_commits:
        commit_short = commit[:7]
        print("Checking commit: {}...".format(commit_short))

        found: Dict[str, Any] = {}

        # Check if there is a hosted app for the current commit
        app_blob = viewer_bucket.blob(f"commit/{commit_short}/index.html")
        if app_blob.exists():
            print("Found web assets commit: {}".format(commit_short))
            found["hosted_app"] = f"https://app.rerun.io/commit/{commit_short}"

        # Check if there are benchmark results
        bench_blob = builds_bucket.blob(f"commit/{commit_short}/bench_results.txt")
        if bench_blob.exists():
            print("Found benchmark results: {}".format(commit_short))
            found["bench_results"] = f"https://build.rerun.io/{bench_blob.name}"

        # Check if there are notebook results
        notebook_blobs = list(builds_bucket.list_blobs(prefix=f"commit/{commit_short}/notebooks"))
        notebooks = [f"https://build.rerun.io/{blob.name}" for blob in notebook_blobs if blob.name.endswith(".html")]
        if notebooks:
            print("Found notebooks for commit: {}".format(commit_short))
            found["notebooks"] = notebooks

        # Get the wheel files for the commit
        wheel_blobs = list(builds_bucket.list_blobs(prefix=f"commit/{commit_short}/wheels"))
        wheels = [f"https://build.rerun.io/{blob.name}" for blob in wheel_blobs if blob.name.endswith(".whl")]
        if wheels:
            print("Found wheels for commit: {}".format(commit_short))
            found["wheels"] = wheels

        if found:
            found["commit"] = commit_short
            found_builds.append(found)

    template_path = os.path.join(os.path.dirname(os.path.relpath(__file__)), "templates/pr_results_summary.html")

    # Render the Jinja template with the found_builds variable
    with open(template_path) as f:
        template = Template(f.read())

    buffer = io.BytesIO(template.render(found_builds=found_builds, pr_number=pr_number).encode("utf-8"))
    buffer.seek(0)

    if upload:
        upload_blob = builds_bucket.blob(f"pull_request/{pr_number}/index.html")
        print("Uploading results to {}".format(upload_blob.name))
        upload_blob.upload_from_file(buffer, content_type="text/html")

        # If there's a {{ pr-build-summary }} string in the PR description, replace it with a link to the summary page.
        pr_description = pull.body
        new_description = pr_description.replace("{{ pr-build-summary }}", f"https://build.rerun.io/pr/{pr_number}")
        pull.edit(body=new_description)


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    parser.add_argument("--upload", action="store_true", help="Upload the summary page to GCS")
    args = parser.parse_args()

    generate_pr_summary(args.github_token, args.github_repository, args.pr_number, args.upload)


if __name__ == "__main__":
    main()
