"""Build a local vector index of DROID camera frames.

For each requested camera the script auto-detects how to get embeddings:

* **Read path** — if the dataset already has a `/camera/{role}/embedding` column
  (DROID registered with `--create-embeddings`), read it straight out of the
  catalog with a DataFusion query. No video decoding, no model needed.
* **Compute path** — otherwise stream the H.264 `VideoStream` via the
  experimental dataloader, decode frames, and embed them with SigLIP-2.

Either way we end up with a columnar `(segment_id, camera, timestamp_ms, vector)`
Arrow table, which we write to a local vector store (LanceDB or Qdrant, see
`--backend`) and index for ANN search.

Run inside the rerun SDK venv, e.g.:

    pixi run uv run ../droid_semantic_search/ingest.py --num-segments 5 --cameras ext1
"""

from __future__ import annotations

import argparse
import itertools
from collections.abc import Iterator
from typing import Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
from vector_store import BACKENDS, DEFAULT_PATHS, open_store

from rerun.catalog import CatalogClient

# DROID camera roles, and the timeline everything is logged on.
ALL_CAMERAS = ("wrist", "ext1", "ext2")
TIMELINE = "real_time"

# DROID is H.264, GOP size 64 at ~15 fps (see the droid-loader). These knobs are only a
# fallback for episodes registered with `--no-optimize` (no keyframe markers): the decoder
# then seeks by a fixed window, so we use 2x the GOP to make sure each window holds a keyframe.
DROID_CODEC = "h264"
DROID_KEYFRAME_INTERVAL = 128
DROID_FPS_ESTIMATE = 15.0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--catalog-url", default="rerun+http://127.0.0.1:51234", help="Rerun catalog URL")
    parser.add_argument("--dataset", default="droid:sample", help="Dataset name in the catalog")
    parser.add_argument("--token", default=None, help="Auth token (if the catalog requires one)")
    parser.add_argument(
        "--cameras",
        default="ext1",
        help="Comma-separated camera roles, or 'all'. Exterior cams (ext1/ext2) give better scene-level matches.",
    )
    parser.add_argument("--num-segments", type=int, default=10, help="Number of segments to index (0 for all)")
    parser.add_argument(
        "--backend",
        choices=BACKENDS,
        default="lance",
        help="Local vector store to write the index to.",
    )
    parser.add_argument(
        "--db-path",
        default=None,
        help="Directory for the local vector DB (default: ./droid_lancedb or ./droid_qdrant per backend).",
    )
    parser.add_argument("--table", default="droid_frames", help="Table/collection name")
    parser.add_argument(
        "--rate-hz",
        type=float,
        default=2.0,
        help="Compute-path only: frames per second to sample from each segment.",
    )
    parser.add_argument(
        "--fetch-batch",
        type=int,
        default=32,
        help="Compute-path only: samples fetched per server round-trip.",
    )
    parser.add_argument(
        "--num-workers",
        type=int,
        default=0,
        help="Compute-path only: DataLoader workers for fetching/decoding.",
    )
    return parser.parse_args()


def resolve_cameras(arg: str) -> list[str]:
    if arg.strip().lower() == "all":
        return list(ALL_CAMERAS)
    roles = [c.strip() for c in arg.split(",") if c.strip()]
    unknown = [r for r in roles if r not in ALL_CAMERAS]
    if unknown:
        raise SystemExit(f"Unknown camera role(s) {unknown}; expected a subset of {ALL_CAMERAS} or 'all'.")
    return roles


def cameras_with_embeddings(schema: object, cameras: list[str]) -> set[str]:
    """Return the subset of *cameras* that already have an embedding entity in the dataset."""
    present = {p.strip("/") for p in schema.entity_paths()}  # type: ignore[attr-defined]
    return {role for role in cameras if f"camera/{role}/embedding" in present}


def _is_list_type(t: pa.DataType) -> bool:
    return bool(pa.types.is_list(t) or pa.types.is_large_list(t) or pa.types.is_fixed_size_list(t))


def _vector_dim(vectors: pa.Array) -> int:
    """Embedding dimensionality of a (variable- or fixed-size) list array."""
    if pa.types.is_fixed_size_list(vectors.type):
        return int(vectors.type.list_size)
    return int(pc.max(pc.list_value_length(vectors)).as_py())


def _embedding_table(role: str, segment_ids: pa.Array, timestamps_ms: pa.Array, vectors: pa.Array) -> pa.Table:
    """Assemble the four index columns into one Arrow table.

    Both ingest paths funnel through here, so they share a single schema and
    `pa.concat_tables` can stitch their results together with no per-row work.
    """
    return pa.table(
        {
            "segment_id": segment_ids,
            "camera": pa.array([role] * len(segment_ids), pa.string()),
            "timestamp_ms": timestamps_ms,
            "vector": vectors,
        },
    )


def _find_embedding_column(schema: pa.Schema, role: str) -> str:
    """Locate the list-typed embedding column for *role* in a query result schema.

    The DROID loader logs embeddings via `rr.AnyValues(embeddings=...)`, so the
    exact column name (e.g. `/camera/ext1/embedding:embeddings`) is derived by
    the platform — discover it rather than hardcoding.
    """
    candidates: list[str] = [f.name for f in schema if _is_list_type(f.type) and "embedding" in f.name.lower()]
    if not candidates:
        raise RuntimeError(f"No list-typed embedding column for camera '{role}'. Columns: {schema.names}")
    for name in candidates:
        if role in name:
            return name
    return candidates[0]


def read_embedding_table(dataset: object, segments: list[str], role: str) -> pa.Table | None:
    """Read pre-computed embeddings for *role* out of the catalog.

    The query result is already columnar Arrow, so we stay in Arrow the whole
    way — filter out missing rows and normalize the column types without ever
    materializing Python row objects.
    """
    view = dataset.filter_segments(segments).filter_contents(  # type: ignore[attr-defined]
        [f"/camera/{role}/embedding", f"/camera/{role}/embedding/**"],
    )
    table = view.reader(index=TIMELINE).to_arrow_table()

    emb_col = _find_embedding_column(table.schema, role)
    if "rerun_segment_id" not in table.schema.names or TIMELINE not in table.schema.names:
        raise RuntimeError(f"Expected 'rerun_segment_id' and '{TIMELINE}' columns, got {table.schema.names}")

    # Columnar equivalent of the per-row `if vec is None or seg is None: continue`.
    keep = pc.and_(pc.is_valid(table.column(emb_col)), pc.is_valid(table.column("rerun_segment_id")))
    table = table.filter(keep)
    if table.num_rows == 0:
        print(f"  [{role}] no pre-computed embeddings")
        return None

    vectors = table.column(emb_col).combine_chunks()
    vectors = vectors.cast(pa.list_(pa.float32(), _vector_dim(vectors)))
    segment_ids = table.column("rerun_segment_id").cast(pa.string())
    # DROID's index timeline is nanosecond timestamps; LanceDB just wants an int.
    timestamps_ms = table.column(TIMELINE).cast(pa.timestamp("ms")).cast(pa.int64())

    out = _embedding_table(role, segment_ids, timestamps_ms, vectors)
    print(f"  [{role}] read {out.num_rows} pre-computed embeddings")
    return out


def _identity_collate(batch: list[Any]) -> list[Any]:
    """Collate that leaves the list of per-sample dicts untouched (picklable for workers)."""
    return batch


def compute_embedding_table(
    dataset: object,
    segments: list[str],
    role: str,
    *,
    rate_hz: float,
    fetch_batch: int,
    num_workers: int,
) -> pa.Table | None:
    """Decode frames for *role* and embed them with SigLIP-2."""
    # Heavy / optional deps are imported lazily so a pure read-path run stays light.
    from embeddings import compute_image_embeddings, load_embedding_model
    from PIL import Image
    from torch.utils.data import DataLoader
    from tqdm import tqdm

    from rerun.experimental.dataloader import (
        DataSource,
        Field,
        FixedRateSampling,
        RerunMapDataset,
        VideoFrameDecoder,
    )

    field_name = f"img_{role}"
    source = DataSource(dataset, segments=segments)  # type: ignore[arg-type]
    fields = {
        field_name: Field(
            f"/camera/{role}:VideoStream:sample",
            decode=VideoFrameDecoder(
                codec=DROID_CODEC,
                keyframe_interval=DROID_KEYFRAME_INTERVAL,
                fps_estimate=DROID_FPS_ESTIMATE,
            ),
        ),
    }
    ds = RerunMapDataset(
        source=source,
        index=TIMELINE,
        fields=fields,
        timeline_sampling=FixedRateSampling(rate_hz=rate_hz),
    )
    total = len(ds)
    print(f"  [{role}] decoding ~{total} frames at {rate_hz} Hz …")

    # The DataLoader earns its keep on the *fetch* side: `batch_size` batches the
    # catalog round-trips, and `num_workers > 0` fans the CPU-bound video decode
    # across worker processes. `shuffle=False` keeps indices in 0..N-1 order, which
    # the running counter in `decoded_frames` relies on for the (segment, timestamp)
    # pairing below.
    loader = DataLoader(
        ds,
        batch_size=fetch_batch,
        shuffle=False,
        num_workers=num_workers,
        collate_fn=_identity_collate,
    )

    def decoded_frames(pbar: tqdm[Any]) -> Iterator[tuple[Image.Image, str, int]]:
        # The loader visits indices 0..N-1 in order, so a running counter pairs each
        # decoded frame back to its (segment, timestamp) via `global_to_local`. The
        # progress bar advances once per sample pulled from the loader (the slow,
        # video-decoding step), including the ones we skip below.
        global_idx = 0
        for batch in loader:
            for sample in batch:
                tensor = sample[field_name]
                seg_meta, idx_val = ds.sample_index.global_to_local(global_idx)
                global_idx += 1
                pbar.update(1)
                if tensor is None:  # target preceded the first keyframe; skip
                    continue
                ts_ms = int(np.datetime64(idx_val).astype("datetime64[ms]").astype(np.int64))  # type: ignore[arg-type]
                rgb = tensor.permute(1, 2, 0).cpu().numpy()  # [C,H,W] uint8 -> [H,W,C]
                yield Image.fromarray(rgb), seg_meta.segment_id, ts_ms

    # Embed the decoded stream chunk-by-chunk so peak memory stays at ~embed_batch
    # frames rather than the whole role. Each chunk becomes one small Arrow table;
    # `pa.concat_tables` stitches them at the end with no per-row work.
    embed_batch = 64
    model, processor = load_embedding_model()
    chunks: list[pa.Table] = []
    with tqdm(total=total, desc=f"[{role}] decode+embed", unit="frame") as pbar:
        for chunk in itertools.batched(decoded_frames(pbar), embed_batch):  # type: ignore[attr-defined, unused-ignore]
            frames = [frame for frame, _, _ in chunk]
            segs = [seg for _, seg, _ in chunk]
            timestamps = [ts_ms for _, _, ts_ms in chunk]
            vectors = compute_image_embeddings(frames, model, processor, batch_size=embed_batch).numpy()
            _, dim = vectors.shape
            vector_col = pa.FixedSizeListArray.from_arrays(pa.array(vectors.reshape(-1), pa.float32()), dim)
            chunks.append(
                _embedding_table(role, pa.array(segs, pa.string()), pa.array(timestamps, pa.int64()), vector_col),
            )

    if not chunks:
        print(f"  [{role}] no frames decoded")
        return None

    out = pa.concat_tables(chunks)
    print(f"  [{role}] computed {out.num_rows} embeddings")
    return out


def main() -> None:
    args = parse_args()
    cameras = resolve_cameras(args.cameras)

    client = CatalogClient(args.catalog_url, token=args.token)
    dataset = client.get_dataset(args.dataset)

    all_segments = dataset.segment_ids()
    segments = all_segments if args.num_segments == 0 else all_segments[: args.num_segments]
    if not segments:
        raise SystemExit(f"Dataset '{args.dataset}' has no segments.")

    have_emb = cameras_with_embeddings(dataset.schema(), cameras)
    print(f"Indexing {len(segments)} segment(s); cameras={cameras}; pre-computed embeddings for {sorted(have_emb)}")

    tables: list[pa.Table] = []
    for role in cameras:
        if role in have_emb:
            table = read_embedding_table(dataset, segments, role)
        else:
            table = compute_embedding_table(
                dataset,
                segments,
                role,
                rate_hz=args.rate_hz,
                fetch_batch=args.fetch_batch,
                num_workers=args.num_workers,
            )
        if table is not None:
            tables.append(table)

    if not tables:
        raise SystemExit("No embeddings produced; nothing to index.")

    db_path = args.db_path or DEFAULT_PATHS[args.backend]
    open_store(args.backend, db_path, args.table).write(pa.concat_tables(tables))


if __name__ == "__main__":
    main()
