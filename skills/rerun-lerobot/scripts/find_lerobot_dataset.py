#!/usr/bin/env -S uv run --quiet --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["huggingface_hub"]
# ///
"""Search the HF Hub for LeRobot datasets and report each candidate's `codebase_version`.

The version is the thing you cannot see from the search results, and it decides whether
"just the first N episodes" is a small download (v2.x) or the whole dataset (v3.0).
Checking it up front costs one request per candidate and saves picking a repo you must
then abandon.
"""

from __future__ import annotations

import argparse
import json
from concurrent.futures import ThreadPoolExecutor

from huggingface_hub import HfApi, hf_hub_download
from huggingface_hub.errors import HfHubHTTPError


def dataset_info(repo_id: str) -> dict | None:
    """Fetch and parse `meta/info.json`, or None if this is not a readable LeRobot dataset."""
    try:
        path = hf_hub_download(repo_id, "meta/info.json", repo_type="dataset")
    except (HfHubHTTPError, OSError):
        return None
    try:
        with open(path) as f:
            info = json.load(f)
    except (OSError, json.JSONDecodeError):
        return None
    return info if isinstance(info, dict) else None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("query", help="free-text search, e.g. a robot or task name")
    parser.add_argument(
        "--robot-type", help="keep only datasets whose robot_type matches (substring, case-insensitive)"
    )
    parser.add_argument("--codebase-version", default="v2", help="keep only versions starting with this (default: v2)")
    parser.add_argument("--want", type=int, default=5, help="stop after this many matches (default: 5)")
    parser.add_argument("--search-limit", type=int, default=40, help="how many search hits to inspect (default: 40)")
    args = parser.parse_args()

    hits = list(HfApi().list_datasets(search=args.query, sort="downloads", limit=args.search_limit))
    # Do not rely on the server's sort order.
    hits.sort(key=lambda hit: hit.downloads or 0, reverse=True)
    if not hits:
        raise SystemExit(f"No datasets found for {args.query!r}")

    with ThreadPoolExecutor(max_workers=8) as pool:
        infos = pool.map(dataset_info, [hit.id for hit in hits])

    print(f"{'repo_id':<55} {'version':<8} {'robot':<12} {'eps':>5} {'fps':>4}  downloads")
    matches = 0
    for hit, info in zip(hits, infos):
        if info is None:
            continue
        version = str(info.get("codebase_version", "?"))
        robot = str(info.get("robot_type", "?"))
        if not version.startswith(args.codebase_version):
            continue
        if args.robot_type and args.robot_type.lower() not in robot.lower():
            continue
        episodes = info.get("total_episodes", "?")
        fps = info.get("fps", "?")
        print(f"{hit.id:<55} {version:<8} {robot:<12} {episodes:>5} {fps:>4}  {hit.downloads or 0}")
        matches += 1
        if matches >= args.want:
            break

    if matches == 0:
        raise SystemExit(
            f"No {args.codebase_version}* dataset among the top {len(hits)} hits for {args.query!r}. "
            "Widen --search-limit, or accept v3.0 and download the whole dataset."
        )


if __name__ == "__main__":
    main()
