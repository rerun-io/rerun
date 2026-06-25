"""Query the DROID frame index with a text prompt or example image and open the best hit in Rerun.

Embeds the query with the SigLIP-2 text *or* image encoder (both share one
vector space, the same one the indexed frame embeddings live in), runs a cosine
nearest-neighbor search over the local LanceDB table, prints the ranked matches,
and mints a `segment_url` deep-link that opens the top result focused on that
frame in the Rerun viewer.

Run inside the rerun SDK venv, e.g.:

    pixi run uv run ../droid_semantic_search/search.py "a robot gripper reaching for a cup"
    pixi run uv run ../droid_semantic_search/search.py --image ./query.jpg
"""

from __future__ import annotations

import argparse
import webbrowser
from datetime import datetime, timedelta, timezone
from typing import cast

import lancedb
from embeddings import (
    EmbeddingModel,
    EmbeddingProcessor,
    compute_image_embeddings,
    get_text_embeddings,
    load_embedding_model,
)

import rerun as rr
from rerun.catalog import CatalogClient

TIMELINE = "real_time"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("query", nargs="?", help="Text prompt to search for")
    parser.add_argument(
        "--image", help="Path to an image to search by (image-to-image); mutually exclusive with the text query"
    )
    parser.add_argument("--catalog-url", default="rerun+http://127.0.0.1:51234", help="Rerun catalog URL")
    parser.add_argument("--dataset", default="droid:sample", help="Dataset name (used to mint the viewer link)")
    parser.add_argument(
        "--login", action="store_true", help="Authenticate with the catalog via rr.login() before connecting"
    )
    parser.add_argument("--lancedb-path", default="./droid_lancedb", help="Directory of the local LanceDB database")
    parser.add_argument("--table", default="droid_frames", help="LanceDB table name")
    parser.add_argument("--top-k", type=int, default=5, help="Number of matches to return")
    parser.add_argument("--window-secs", type=float, default=2.0, help="Time window around the matched frame to show")
    parser.add_argument(
        "--open",
        default=True,
        action=argparse.BooleanOptionalAction,
        help="Open the top hit in the viewer",
    )
    return parser.parse_args()


def embed_image_query(path: str, model: EmbeddingModel, processor: EmbeddingProcessor) -> list[float]:
    """Embed an image file for image-to-image search.

    Wired to the `--image` flag: text and image features share one SigLIP-2
    vector space, so a query image retrieves frames the same way a text prompt does.
    """
    from PIL import Image

    image = Image.open(path).convert("RGB")
    vector = compute_image_embeddings([image], model, processor).numpy()[0]
    return cast("list[float]", vector.tolist())


def viewer_url(dataset: object, segment_id: str, timestamp_ms: int, window_secs: float) -> str:
    ts = datetime.fromtimestamp(timestamp_ms / 1000, tz=timezone.utc)
    half = timedelta(seconds=window_secs / 2)
    return dataset.segment_url(segment_id, timeline=TIMELINE, start=ts - half, end=ts + half)  # type: ignore[attr-defined, no-any-return]


def main() -> None:
    args = parse_args()

    if bool(args.query) == bool(args.image):
        raise SystemExit("Provide exactly one of: a text query or --image <path>.")

    model, processor = load_embedding_model()
    if args.image:
        query_vec = embed_image_query(args.image, model, processor)
        query_label = f"image {args.image}"
    else:
        query_vec = get_text_embeddings(args.query, model, processor).numpy()[0].tolist()
        query_label = args.query

    tbl = lancedb.connect(args.lancedb_path).open_table(args.table)
    hits = tbl.search(query_vec).metric("cosine").limit(args.top_k).to_list()
    if not hits:
        raise SystemExit("No matches found — is the table populated? Run ingest.py first.")

    print(f'\nTop {len(hits)} matches for: "{query_label}"\n')
    print(f"{'#':>2}  {'sim':>5}  {'camera':<6}  {'timestamp (UTC)':<24}  segment")
    for rank, hit in enumerate(hits, start=1):
        similarity = 1.0 - float(hit["_distance"])  # cosine distance -> similarity
        ts_iso = datetime.fromtimestamp(hit["timestamp_ms"] / 1000, tz=timezone.utc).isoformat(timespec="milliseconds")
        print(f"{rank:>2}  {similarity:>5.3f}  {hit['camera']:<6}  {ts_iso:<24}  {hit['segment_id']}")

    # Mint a deep-link into the viewer for the best match.
    if args.login:
        rr.login()
    client = CatalogClient(args.catalog_url)
    dataset = client.get_dataset(args.dataset)
    best = hits[0]
    url = viewer_url(dataset, best["segment_id"], best["timestamp_ms"], args.window_secs)
    print(f"\nTop hit in viewer:\n{url}")
    if args.open:
        webbrowser.open(url)


if __name__ == "__main__":
    main()
