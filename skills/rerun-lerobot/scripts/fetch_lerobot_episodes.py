#!/usr/bin/env -S uv run --quiet --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["huggingface_hub"]
# ///
"""Download the first N episodes of a v2.x LeRobot dataset and trim `meta/` to match.

Prints the dataset directory as the last line, ready to hand to the viewer.

Two things this gets right that a hand-rolled `snapshot_download` does not:

- The destination starts empty, so episodes left by an earlier run with a different N
  cannot survive and silently inflate the result.
- `meta/` is trimmed to the episodes actually on disk, so the importer does not warn
  once per absent episode.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

from huggingface_hub import hf_hub_download, snapshot_download
from huggingface_hub.errors import HfHubHTTPError


def load_info(repo_id: str) -> dict:
    """Read `meta/info.json` from the Hub without downloading the dataset itself."""
    try:
        path = hf_hub_download(repo_id, "meta/info.json", repo_type="dataset")
    except (HfHubHTTPError, OSError) as err:
        sys.exit(
            f"Cannot read meta/info.json — the repo may be missing, gated, or not a LeRobot dataset: {err}\n"
            f"Repo: {repo_id}"
        )
    with open(path) as f:
        return json.load(f)


def prepare_dest(dest: Path) -> None:
    """Empty `dest`, refusing anything that is not an existing LeRobot dataset."""
    if not dest.exists():
        return
    is_dataset = (dest / "meta" / "info.json").is_file()
    if not is_dataset and any(dest.iterdir()):
        sys.exit(f"Refusing to delete a directory that is not a LeRobot dataset\nPath: {dest}")
    shutil.rmtree(dest)


def trim_meta(meta: Path, episodes: int) -> None:
    """Cut the episode lists and the counts in `info.json` down to the first `episodes`."""
    kept = []
    episodes_jsonl = meta / "episodes.jsonl"
    if episodes_jsonl.is_file():
        lines = [line for line in episodes_jsonl.read_text().splitlines() if line.strip()]
        kept = [json.loads(line) for line in lines[:episodes]]
        episodes_jsonl.write_text("".join(json.dumps(entry) + "\n" for entry in kept))

    stats_jsonl = meta / "episodes_stats.jsonl"
    if stats_jsonl.is_file():
        lines = [line for line in stats_jsonl.read_text().splitlines() if line.strip()]
        stats_jsonl.write_text("".join(line + "\n" for line in lines[:episodes]))

    info_json = meta / "info.json"
    info = json.loads(info_json.read_text())
    videos_per_episode = sum(1 for feature in info.get("features", {}).values() if feature.get("dtype") == "video")
    info["total_episodes"] = episodes
    info["total_videos"] = episodes * videos_per_episode
    info["splits"] = {"train": f"0:{episodes}"}
    if kept:
        info["total_frames"] = sum(entry.get("length", 0) for entry in kept)
    info_json.write_text(json.dumps(info, indent=4))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo_id", help="HF dataset repo id, e.g. <user>/<dataset>")
    parser.add_argument("--episodes", type=int, default=5, help="how many episodes, from 0 (default: 5)")
    parser.add_argument(
        "--dest-root",
        type=Path,
        default=Path.home() / "lerobot",
        help="parent directory; the dataset lands in a subdirectory named after the repo (default: ~/lerobot)",
    )
    args = parser.parse_args()

    if args.episodes < 1:
        sys.exit("--episodes must be at least 1")

    info = load_info(args.repo_id)
    version = str(info.get("codebase_version", "?"))
    if not version.startswith("v2"):
        sys.exit(
            f"{args.repo_id} is codebase_version {version}, which concatenates every episode into shared files, "
            "so a subset costs the whole dataset. Pick a v2.x dataset, or download it in full yourself."
        )

    total = info.get("total_episodes")
    if isinstance(total, int) and args.episodes > total:
        sys.exit(f"{args.repo_id} has only {total} episodes, but --episodes is {args.episodes}")

    dest = args.dest_root / args.repo_id.replace("/", "__")
    prepare_dest(dest)

    snapshot_download(
        repo_id=args.repo_id,
        repo_type="dataset",
        local_dir=dest,
        allow_patterns=["meta/*"] + [f"**/episode_{i:06d}.*" for i in range(args.episodes)],
    )
    trim_meta(dest / "meta", args.episodes)

    got = sorted(path.name for path in (dest / "data").rglob("*.parquet"))
    want = [f"episode_{i:06d}.parquet" for i in range(args.episodes)]
    if got != want:
        sys.exit(f"Downloaded the wrong episodes\nExpected: {want}\nGot: {got}\nPath: {dest}")

    print(dest)


if __name__ == "__main__":
    main()
