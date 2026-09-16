#!/usr/bin/env -S uv run --quiet --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["huggingface_hub"]
# ///
"""Search the HF Hub for LeRobot datasets and report what each candidate costs to fetch.

Neither of the two facts that decide the cost is visible in the search results:

- `codebase_version` decides whether "just the first N episodes" is a partial download
  (v2.x) or the whole dataset (v3.0).
- How camera frames are stored decides the size. A `video` feature is a separate mp4;
  an `image` feature is raw frames inline in the episode parquet, which is roughly two
  orders of magnitude larger per episode.

Checking both up front costs two requests per candidate and saves picking a repo you must
then abandon, or waiting out a download an order of magnitude larger than you expected.
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


def frame_storage(info: dict) -> str:
    """How the camera frames are stored: `"mp4"` beside the parquet, or `"inline"` within it.

    A dataset may use both, one per camera, and inline frames then decide the answer.

    Copied in `fetch_lerobot_episodes.py`: each script is a self-contained `uv run --script` file,
    which a shared module would end.
    """
    dtypes = {str(feature.get("dtype")) for feature in info.get("features", {}).values()}
    if "image" in dtypes:
        return "inline"
    if "video" in dtypes:
        return "mp4"
    return "none"


def repo_bytes(repo_id: str) -> int | None:
    """Total size of every file in the repo, or None if the Hub does not report it."""
    try:
        siblings = HfApi().dataset_info(repo_id, files_metadata=True).siblings or []
    except (HfHubHTTPError, OSError):
        return None
    sizes = [sibling.size for sibling in siblings if sibling.size is not None]
    return sum(sizes) if sizes else None


def per_episode_mb(repo_id: str, info: dict) -> str:
    """Approximate megabytes per episode, as the whole repo divided by its episode count."""
    total = repo_bytes(repo_id)
    episodes = info.get("total_episodes")
    if total is None or not isinstance(episodes, int) or episodes < 1:
        return "?"
    return f"{total / episodes / 1e6:.0f}"


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
        infos = list(pool.map(dataset_info, [hit.id for hit in hits]))

    print(f"{'repo_id':<55} {'version':<8} {'robot':<12} {'eps':>5} {'fps':>4} {'frames':>7} {'MB/ep':>6}  downloads")
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
        frames = frame_storage(info)
        size = per_episode_mb(hit.id, info)
        print(
            f"{hit.id:<55} {version:<8} {robot:<12} {episodes:>5} {fps:>4} {frames:>7} {size:>6}  {hit.downloads or 0}"
        )
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
