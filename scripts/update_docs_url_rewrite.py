#!/usr/bin/env python3

"""
This script updates the GCP load balancer URL map to add URL rewrite rules which
redirect requests from `/docs/{python,js}/stable/*` to `/docs/{python,js}/$VERSION/*`.

Installation
------------

Requires the following packages:
  google-cloud-compute>=1.20.0

Before running, you must authenticate via the Google Cloud CLI:
- Install it (https://cloud.google.com/sdk/docs/install)
- Set up credentials: gcloud auth application-default login

Usage
-----

Test with dry-run first:

    uv run --group dev scripts/update_docs_url_rewrite.py --version 0.28.2 --language python --dry-run

Then apply the changes:

    uv run --group dev scripts/update_docs_url_rewrite.py --version 0.28.2 --language python
"""

from __future__ import annotations

import argparse
import difflib
import json
import logging
import sys

from google.api_core import exceptions as google_exceptions
from google.api_core import retry
from google.api_core.retry.retry_base import if_transient_error
from google.cloud import compute_v1


def format_url_map(url_map: compute_v1.UrlMap) -> str:
    """Format URL map as a readable JSON string."""
    return json.dumps(compute_v1.UrlMap.to_dict(url_map), indent=2, sort_keys=True)


def is_resource_not_ready_error(exc: Exception) -> bool:
    """Check if an exception is a 'resource not ready' error that should be retried."""

    # fall back to the default `if_transient_error` to ensure those are still retried
    return if_transient_error(exc) or isinstance(exc, google_exceptions.BadRequest) and "is not ready" in str(exc)


def update_url_map_rewrite_rules(project: str, version: str, language: str, dry_run: bool = False) -> None:
    """
    Update the URL map to add or update rewrite rules for stable -> version redirects.

    Parameters
    ----------
    project : str
        The GCP project ID.
    version : str
        The version to redirect stable to (e.g., "0.28.2").
    language : str
        The language to update ("python" or "js").
    dry_run : bool
        If True, show what would be done without making changes.
    """
    client = compute_v1.UrlMapsClient()

    balancer_name = "rerun-docs-balancer"

    logging.info(f"Fetching URL map for balancer {balancer_name}")
    url_map = client.get(project=project, url_map=balancer_name)

    # Capture the "before" state
    before_state = format_url_map(url_map)

    backend_bucket = "rerun-docs-backend"
    backend_url = f"https://www.googleapis.com/compute/v1/projects/{project}/global/backendBuckets/{backend_bucket}"

    # Initialize pathMatchers if it doesn't exist
    if not url_map.path_matchers:
        url_map.path_matchers = []

    # Find or create the default path matcher
    path_matcher_idx = None
    for idx, pm in enumerate(url_map.path_matchers):
        if pm.name == "path-matcher-1":
            path_matcher_idx = idx
            break

    if path_matcher_idx is None:
        logging.info("Creating new path matcher")
        path_matcher = compute_v1.types.PathMatcher(name="path-matcher-1", default_service=backend_url, path_rules=[])
        url_map.path_matchers.append(path_matcher)
        path_matcher_idx = len(url_map.path_matchers) - 1

    # Work directly with the path matcher in url_map (protobuf makes a copy on append)
    pm = url_map.path_matchers[path_matcher_idx]

    # Remove existing stable rewrite rule for the specified language
    stable_prefix = f"/docs/{language}/stable"
    pm.path_rules[:] = [
        rule for rule in pm.path_rules if not any(path.startswith(stable_prefix) for path in rule.paths)
    ]

    logging.info(f"Adding rewrite rule: {language}/stable -> {language}/{version}")

    # Create new rewrite rule for the specified language
    new_rule = compute_v1.types.PathRule(
        paths=[f"/docs/{language}/stable/*"],
        service=backend_url,
        route_action=compute_v1.types.HttpRouteAction(
            url_rewrite=compute_v1.types.UrlRewrite(path_prefix_rewrite=f"/docs/{language}/{version}/")
        ),
    )

    # Insert at the beginning (higher priority)
    pm.path_rules.insert(0, new_rule)

    # Ensure there's a host rule pointing to the path matcher
    # Without a host rule, the path matcher is never evaluated
    if not url_map.host_rules:
        logging.info("Adding host rule to connect path matcher")
        url_map.host_rules = [compute_v1.types.HostRule(hosts=["ref.rerun.io"], path_matcher="path-matcher-1")]

    # Capture the "after" state
    after_state = format_url_map(url_map)

    # Generate diff
    before_lines = before_state.splitlines(keepends=True)
    after_lines = after_state.splitlines(keepends=True)
    diff = difflib.unified_diff(
        before_lines,
        after_lines,
        fromfile=f"{balancer_name} (before)",
        tofile=f"{balancer_name} (after)",
        lineterm="",
    )

    diff_output = "".join(diff)

    if diff_output:
        print("\nURL Map Configuration Diff:")
        print("=" * 80)
        print(diff_output)
        print("=" * 80)
    else:
        logging.info("No changes to URL map (already up to date)")
        return

    if dry_run:
        logging.info("\nDRY RUN: Would apply the above changes")
        return

    logging.info(f"\nApplying changes to URL map: {balancer_name}")

    operation = client.update(
        project=project,
        url_map=balancer_name,
        url_map_resource=url_map,
        # Retry in case of "Resource is not ready" errors, which may occur when this script
        # is called in parallel from different jobs.
        retry=retry.Retry(predicate=is_resource_not_ready_error),
    )

    logging.info("Waiting for operation to complete…")
    operation.result()

    logging.info("✓ URL map updated successfully")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument("--version", type=str, required=True, help="The version to redirect stable to (e.g., '0.28.2')")
    parser.add_argument(
        "--language", type=str, required=True, choices=["python", "js"], help="The language to update (python or js)"
    )
    parser.add_argument("--project", type=str, default="rerun-open", help="GCP project ID (default: rerun-open)")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be done without making changes")
    parser.add_argument("--debug", action="store_true", help="Enable debug logging")

    args = parser.parse_args()

    if args.debug:
        logging.basicConfig(level=logging.DEBUG)
    else:
        logging.basicConfig(level=logging.INFO)

    try:
        update_url_map_rewrite_rules(args.project, args.version, args.language, args.dry_run)
    except Exception as e:
        logging.error(f"Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
