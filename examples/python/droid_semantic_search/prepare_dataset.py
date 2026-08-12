"""Register a few DROID episodes to a local Rerun catalog so the rest of the example has data to index.

Two sources, auto-selected (override with `--source`):

* **Bundled** — the `tests/assets/rrd/sample_5` episodes shipped in the Rerun repo (via git-LFS).
  Used automatically in a monorepo checkout: no download, works offline.
* **Hugging Face** — a few episodes from the [`rerun/droid_sample`](https://huggingface.co/datasets/rerun/droid_sample)
  dataset, downloaded into `./data`. Used when the bundled episodes aren't available
  (e.g. a standalone sparse-checkout of just this example).

Either way the episodes are registered to the catalog as a dataset (default name `droid:sample`).
They carry H.264 `VideoStream`s but no pre-computed embeddings, so `ingest.py` will take its
(slower) compute path: decode frames and embed them with SigLIP-2.

Episodes are optimized first to derive keyframe markers, so the compute path can decode
frames (DROID doesn't log the markers the decoder needs). Pass `--no-optimize` to skip.

Run inside the rerun SDK venv, with a `rerun server` running in another terminal, e.g.:

    uv run python prepare_dataset.py
"""

from __future__ import annotations

import argparse
from dataclasses import replace
from pathlib import Path

from tqdm import tqdm

import rerun as rr
from rerun.experimental import OptimizationProfile, RrdReader

# `tests/assets/rrd/sample_5`, relative to this file at `examples/python/droid_semantic_search/`.
BUNDLED_SAMPLE_DIR = Path(__file__).resolve().parents[3] / "tests" / "assets" / "rrd" / "sample_5"
DEFAULT_REPO_ID = "rerun/droid_sample"
DEFAULT_OUTPUT_DIR = Path(__file__).resolve().parent / "data"
DEFAULT_OPTIMIZED_DIR = Path(__file__).resolve().parent / "optimized"
DEFAULT_DATASET_NAME = "droid:sample"
DEFAULT_CATALOG_URL = "rerun+http://127.0.0.1:51234"

_LFS_POINTER_PREFIX = b"version https://git-lfs.github.com/spec/v1"


def _is_lfs_pointer(path: Path) -> bool:
    """True if *path* is an un-pulled git-LFS pointer file rather than the real RRD."""
    with path.open("rb") as f:
        return f.read(len(_LFS_POINTER_PREFIX)) == _LFS_POINTER_PREFIX


def bundled_episode_paths() -> list[Path]:
    """Return the bundled sample_5 RRDs (sorted), or an empty list if they're not available.

    Raises if the directory exists but the files are un-pulled git-LFS pointers — that's a
    recoverable monorepo setup issue worth surfacing rather than silently working around.
    """
    if not BUNDLED_SAMPLE_DIR.is_dir():
        return []
    rrds = sorted(BUNDLED_SAMPLE_DIR.glob("*.rrd"))
    if not rrds:
        return []
    if any(_is_lfs_pointer(p) for p in rrds):
        raise SystemExit(
            f"The bundled DROID episodes in {BUNDLED_SAMPLE_DIR} are un-pulled git-LFS pointers.\n"
            "Fetch them with `git lfs install && git lfs pull`, or pass `--source huggingface` to download instead.",
        )
    return rrds


def download_episodes(repo_id: str, num_episodes: int, dest: Path) -> list[Path]:
    """Download the first *num_episodes* (0 for all) `.rrd` files from *repo_id* into *dest*.

    `snapshot_download` renders its own per-file progress bars, so the user sees the download advance.
    """
    from huggingface_hub import HfApi, snapshot_download

    files = sorted(f for f in HfApi().list_repo_files(repo_id, repo_type="dataset") if f.endswith(".rrd"))
    if not files:
        raise SystemExit(f"No .rrd files found in '{repo_id}'.")
    files = files if num_episodes == 0 else files[:num_episodes]

    print(f"Downloading {len(files)} episode(s) from '{repo_id}' to {dest} …")
    local_dir = snapshot_download(repo_id=repo_id, repo_type="dataset", allow_patterns=files, local_dir=dest)
    return [Path(local_dir) / f for f in files]


def resolve_episodes(source: str, *, num_episodes: int, repo_id: str, output_dir: Path) -> list[Path]:
    """Pick the episode RRDs to register, per the requested *source* (`auto`/`bundled`/`huggingface`)."""
    if source in ("auto", "bundled"):
        bundled = bundled_episode_paths()
        if bundled:
            paths = bundled if num_episodes == 0 else bundled[:num_episodes]
            print(f"Using {len(paths)} bundled episode(s) from {BUNDLED_SAMPLE_DIR}")
            return paths
        if source == "bundled":
            raise SystemExit(f"No bundled episodes found at {BUNDLED_SAMPLE_DIR}; pass `--source huggingface`.")
        print(f"Bundled episodes not found at {BUNDLED_SAMPLE_DIR}; downloading from the Hub instead.")

    return download_episodes(repo_id, num_episodes, output_dir)


def optimize_episodes(rrd_paths: list[Path], dest_dir: Path) -> list[Path]:
    """Derive `VideoStream:is_keyframe` markers (DROID doesn't log them) so the decoder can seek.

    Writes a fixed copy of each episode to *dest_dir* and returns the new paths. IDs are
    preserved, so catalog segment IDs and `segment_url` links are unchanged.
    """
    dest_dir.mkdir(parents=True, exist_ok=True)
    profile = replace(OptimizationProfile.OBJECT_STORE, fix_keyframe=True)

    optimized: list[Path] = []
    for src in tqdm(rrd_paths, desc="Optimizing", unit="episode"):
        reader = RrdReader(src)
        recordings = reader.recordings()
        if len(recordings) != 1:
            raise SystemExit(f"Expected one recording in {src}, found {len(recordings)}.")
        entry = recordings[0]

        store = reader.stream(store=entry).collect(optimize=profile)
        dst = dest_dir / src.name
        store.write_rrd(dst, application_id=entry.application_id, recording_id=entry.recording_id)
        optimized.append(dst)

    return optimized


def register_to_catalog(rrd_paths: list[Path], *, catalog_url: str, dataset_name: str) -> None:
    """Register per-episode RRDs to a catalog server instance.

    Uses absolute `file://` URIs so the catalog can read the RRDs directly from the local filesystem.
    Streams `iter_results()` so a progress bar advances as each segment finishes, rather than blocking
    silently on `wait()`.
    """
    print(f"\nRegistering {len(rrd_paths)} episode(s) to {catalog_url} as dataset '{dataset_name}' …")
    client = rr.catalog.CatalogClient(catalog_url)
    dataset = client.create_dataset(dataset_name, exist_ok=True)

    uris = [f"file://{p.resolve()}" for p in rrd_paths]
    on_duplicate = rr.catalog.OnDuplicateSegmentLayer(rr.catalog.OnDuplicateSegmentLayer.REPLACE)
    handle = dataset.register(uris, on_duplicate=on_duplicate)

    failures: list[str] = []
    for result in tqdm(handle.iter_results(), total=len(uris), desc="Registering", unit="segment"):
        if result.is_error:
            failures.append(f"{result.uri}: {result.error}")

    if failures:
        joined = "\n  ".join(failures)
        raise SystemExit(f"Failed to register {len(failures)} of {len(uris)} episode(s):\n  {joined}")
    print("  registration done")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--source",
        choices=("auto", "bundled", "huggingface"),
        default="auto",
        help="Where to get episodes: 'bundled' (in-repo sample_5), 'huggingface' (download), "
        "or 'auto' (bundled if available, else download). Default: auto.",
    )
    parser.add_argument(
        "--repo-id",
        default=DEFAULT_REPO_ID,
        help=f"Hugging Face dataset repo id, for the download path (default: {DEFAULT_REPO_ID}).",
    )
    parser.add_argument(
        "--num-episodes",
        type=int,
        default=5,
        help="Number of episodes to register (0 for all). The full Hub dataset is ~3.3 GB.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Directory to download episode RRDs into, for the download path (default: {DEFAULT_OUTPUT_DIR}).",
    )
    parser.add_argument(
        "--optimize",
        default=True,
        action=argparse.BooleanOptionalAction,
        help="Derive keyframe markers before registering, so ingest.py can decode frames "
        "(~100%% yield vs ~25%%). Use --no-optimize to register episodes as-is.",
    )
    parser.add_argument(
        "--optimized-dir",
        type=Path,
        default=DEFAULT_OPTIMIZED_DIR,
        help=f"Directory to write optimized episode RRDs into (default: {DEFAULT_OPTIMIZED_DIR}).",
    )
    parser.add_argument(
        "--catalog-url",
        default=DEFAULT_CATALOG_URL,
        help="Rerun catalog URL to register episodes with. Pass an empty string to skip registration.",
    )
    parser.add_argument(
        "--dataset-name",
        default=DEFAULT_DATASET_NAME,
        help=f"Name of the dataset to create/use in the catalog (default: {DEFAULT_DATASET_NAME}).",
    )
    args = parser.parse_args()

    rrd_paths = resolve_episodes(
        args.source,
        num_episodes=args.num_episodes,
        repo_id=args.repo_id,
        output_dir=args.output_dir,
    )

    if args.optimize:
        print(f"\nOptimizing {len(rrd_paths)} episode(s) into {args.optimized_dir} …")
        rrd_paths = optimize_episodes(rrd_paths, args.optimized_dir)

    if args.catalog_url:
        register_to_catalog(rrd_paths, catalog_url=args.catalog_url, dataset_name=args.dataset_name)
    else:
        print(f"Skipping registration (empty --catalog-url). Episodes: {[str(p) for p in rrd_paths]}")


if __name__ == "__main__":
    main()
