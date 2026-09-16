#!/usr/bin/env -S uv run --quiet --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["huggingface_hub"]
# ///
"""Download the first N episodes of a v2.x LeRobot dataset and trim `meta/` to match.

Prints a one-line summary of what landed, then the dataset directory as the last line,
ready to hand to the viewer.

Three things this gets right that a hand-rolled `snapshot_download` does not:

- The destination starts empty, so episodes left by an earlier run with a different N
  cannot survive and silently inflate the result.
- `meta/` is trimmed to the episodes actually on disk, so the importer does not warn
  once per absent episode.
- The size is reported, so "the first five episodes" of a dataset that stores its camera
  frames inline in the parquet is not a silent multi-gigabyte download.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
from fnmatch import fnmatch
from pathlib import Path

from huggingface_hub import HfApi, hf_hub_download, snapshot_download
from huggingface_hub.errors import HfHubHTTPError
from huggingface_hub.utils import disable_progress_bars


def frame_storage(info: dict) -> str:
    """How the camera frames are stored: `"mp4"` beside the parquet, or `"inline"` within it.

    A dataset may use both, one per camera. Inline frames are what make a download expensive, so
    they decide the answer: reporting such a dataset as `"mp4"` would hide the very thing the
    caller is being warned about.

    Copied in `find_lerobot_dataset.py`: each script is a self-contained `uv run --script` file,
    which a shared module would end.
    """
    dtypes = {str(feature.get("dtype")) for feature in info.get("features", {}).values()}
    if "image" in dtypes:
        return "inline"
    if "video" in dtypes:
        return "mp4"
    return "none"


def bytes_to_download(repo_id: str, patterns: list[str]) -> int | None:
    """Size of the files `snapshot_download` would fetch, or None if the HF Hub does not report it.

    Worth one request: an `image`-dtype dataset keeps its camera frames inline in the episode
    parquet, so five episodes can be gigabytes where an mp4 dataset would be tens of megabytes.
    """
    try:
        siblings = HfApi().dataset_info(repo_id, files_metadata=True).siblings or []
    except (HfHubHTTPError, OSError):
        return None
    wanted = [
        sibling
        for sibling in siblings
        if any(fnmatch(sibling.rfilename, pattern) for pattern in patterns) and sibling.size is not None
    ]
    return sum(sibling.size for sibling in wanted) if wanted else None


def dataset_bytes(dest: Path) -> int:
    """Size of the dataset itself, ignoring the `.cache/huggingface` bookkeeping beside it."""
    return sum(
        path.stat().st_size
        for path in dest.rglob("*")
        if path.is_file() and ".cache" not in path.relative_to(dest).parts
    )


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

    # Don't show progress bars in non-interactive shells.
    if not sys.stderr.isatty():
        disable_progress_bars()

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

    patterns = ["meta/*"] + [f"**/episode_{i:06d}.*" for i in range(args.episodes)]

    # Print the download size upfront, so a caller can cancel if it's too large.
    wanted_bytes = bytes_to_download(args.repo_id, patterns)
    size = "unknown size" if wanted_bytes is None else f"{wanted_bytes / 1e9:.2f} GB"
    print(
        f"Fetching {args.episodes} episode(s) of {args.repo_id}: {size}, camera frames stored {frame_storage(info)}",
        file=sys.stderr,
    )

    snapshot_download(
        repo_id=args.repo_id,
        repo_type="dataset",
        local_dir=dest,
        allow_patterns=patterns,
    )
    trim_meta(dest / "meta", args.episodes)

    got = sorted(path.name for path in (dest / "data").rglob("*.parquet"))
    want = [f"episode_{i:06d}.parquet" for i in range(args.episodes)]
    if got != want:
        sys.exit(f"Downloaded the wrong episodes\nExpected: {want}\nGot: {got}\nPath: {dest}")

    size_gb = dataset_bytes(dest) / 1e9
    print(f"{args.episodes} episode(s), {size_gb:.2f} GB, camera frames stored {frame_storage(info)}")
    print(dest)


if __name__ == "__main__":
    main()
