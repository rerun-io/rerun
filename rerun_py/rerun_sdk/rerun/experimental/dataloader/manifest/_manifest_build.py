"""Build a sampling-manifest table by unrolling one epoch of blockwise sampling."""

# Scans the source once to resolve every sample's real decode ranges and drop
# invalid samples, then unrolls one epoch of the blockwise sampling procedure —
# the strategy's fetch order plus the buffer emission shuffle — into a single
# parquet table. The unroll reuses the exact runtime primitives (`ShuffleStrategy`,
# `_fetch_blocks`, `ShuffleBuffer`) over sample IDs, so build and runtime can
# never diverge, and no decoding happens here.

from __future__ import annotations

import itertools
from collections import defaultdict
from dataclasses import dataclass
from functools import partial
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc

from rerun._tracing import set_current_span_attributes, tracing_scope, with_tracing

from ..._query_metrics import QueryMetrics, query_metrics
from .._sample_index import SampleIndex, SegmentMetadata
from .._shuffle import NoShuffle, ShuffleBuffer, ShuffleStrategy, _contiguous_shard, _fetch_blocks
from .._utils import (
    _fetch_prior_keyframes,
    _run_parallel,
    _WorkerConnection,
    is_video_field,
)
from ._manifest import (
    COL_ANCHOR,
    COL_EMIT_RANK,
    COL_FETCH_GROUP,
    COL_RANK,
    COL_SEGMENT_ID,
    COL_WORKER,
    MANIFEST_FORMAT_VERSION,
    RANGE_HI,
    RANGE_LO,
    ManifestMeta,
    _metadata_to_arrow,
)

if TYPE_CHECKING:
    from rerun.catalog._entry import DatasetEntry

    from .._config import DataSource, Field
    from .._sample_index import FixedRateSampling

# Sorted, unique index values (ns / step counts) per segment: `{segment_id: values}`.
_SegmentIndices = dict[str, np.ndarray]

# Max concurrent scan queries against the server during the validity scan.
_SCAN_MAX_WORKERS = 8


@dataclass(frozen=True)
class _ScanResult:
    """Per-segment index values gathered by the validity scan."""

    keyframes: dict[str, _SegmentIndices]  # {field_key: {segment_id: prior-keyframe positions}}
    real_by_entity: dict[str, _SegmentIndices]  # {entity_path: {segment_id: observed index values}}


@dataclass(frozen=True)
class _ResolvedRows:
    """Valid samples in canonical (segment, ascending) order, with each field's decode range."""

    segment_ids: pa.Array  # dictionary<int32 -> string>: an int32 code per row, one id string per segment
    anchors: np.ndarray  # int64
    field_ranges: dict[str, tuple[np.ndarray, np.ndarray]]  # {field_key: (lo, hi) int64 arrays}


@with_tracing("build_manifest_table")
def build_manifest_table(
    source: DataSource,
    index: str,
    fields: dict[str, Field],
    *,
    timeline_sampling: FixedRateSampling | None = None,
    fetch_block_size: int = 128,
    num_ranks: int = 1,
    num_workers_per_rank: int = 1,
    required_fields: list[str] | None = None,
    scan_max_workers: int | None = None,
) -> pa.Table:
    """
    Build a validity-checked, **unshuffled** sampling-manifest table for one epoch.

    Scans the source once to capture each non-video field's **actual observed**
    index values and each video field's sparse keyframe index (not the algebraic
    grid), drops samples whose required fields have no usable data, then unrolls
    one epoch in natural order (see [`NoShuffle`][rerun.experimental.dataloader.NoShuffle])
    into per-`(rank, worker)` `fetch_group` and `emit_rank` columns. Re-order it later with
    [`schedule_samples`][]. Returns the Arrow table (header in the schema
    metadata); [`Manifest.generate`][rerun.experimental.dataloader.Manifest.generate] wraps it into a
    manifest, which is persisted with
    [`Manifest.write_parquet`][rerun.experimental.dataloader.Manifest.write_parquet].

    Parameters
    ----------
    source, index, fields, timeline_sampling
        Describe the sample space, exactly as for
        [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset].
    fetch_block_size
        Samples per co-fetch / co-decode block (one `fetch_group`).
    num_ranks, num_workers_per_rank
        DataLoader topology the `(rank, worker)` assignment is frozen for.
    required_fields
        Field keys that must resolve to real data for a sample to be kept;
        defaults to all fields.
    scan_max_workers
        Max concurrent scan queries against the server. Higher values speed up
        the (dominant) scan of a large dataset at the cost of more server load
        and memory; defaults to `8`.

    """
    required = set(required_fields) if required_fields is not None else set(fields)

    sample_index = SampleIndex.build(source, index, fields, timeline_sampling=timeline_sampling)

    conn = _WorkerConnection(
        catalog_url=source.dataset.catalog.url,
        dataset_name=source.dataset.name,
        fields=fields,
    )
    view, _ = conn.ensure()

    # The scan only needs each segment's largest target (for the prior-keyframe query),
    # not every sample, so pass one representative per segment rather than materializing
    # the whole sample space. `_resolve_rows` then walks the segments itself.
    step = sample_index.ns_per_sample or 1
    segment_maxes = [
        (seg, int(seg.index_start) + (seg.num_samples - 1) * step)
        for seg in sample_index.segments
        if seg.num_samples > 0
    ]
    scan = _scan(
        view=view,
        index=index,
        fields=fields,
        sample_index=sample_index,
        segment_maxes=segment_maxes,
        required=required,
        max_workers=scan_max_workers if scan_max_workers is not None else _SCAN_MAX_WORKERS,
    )

    rows = _resolve_rows(
        fields=fields,
        sample_index=sample_index,
        scan=scan,
        required=required,
    )

    # A freshly built manifest is unshuffled: natural fetch order, no emission
    # buffer (`NoShuffle` never defines one).
    strategy = NoShuffle()
    table = schedule_samples(
        _sample_table(rows, list(fields)),
        strategy=strategy,
        fetch_block_size=fetch_block_size,
        num_ranks=num_ranks,
        num_workers_per_rank=num_workers_per_rank,
        seed=0,
    )

    meta = ManifestMeta(
        format_version=MANIFEST_FORMAT_VERSION,
        dataset_name=source.dataset.name,
        dataset_id=str(source.dataset.id),
        index_name=index,
        ns_per_sample=sample_index.ns_per_sample,
        ns_dtype=sample_index.ns_dtype,
        recipe={key: f.to_recipe() for key, f in fields.items()},
        required_fields=sorted(required),
        fetch_block_size=fetch_block_size,
        buffer_size=None,
        min_fill=None,
        num_ranks=num_ranks,
        num_workers_per_rank=num_workers_per_rank,
        seed=0,
        shuffle_strategy=strategy.RECIPE_TAG,
    )
    return table.replace_schema_metadata(_metadata_to_arrow(meta))


# --------------------------------------------------------------------------------------
# Scan: resolve the validated sample space (the expensive part).
# --------------------------------------------------------------------------------------


@with_tracing("build_manifest_table._scan")
def _scan(
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    segment_maxes: list[tuple[SegmentMetadata, int]],
    required: set[str],
    max_workers: int,
) -> _ScanResult:
    """Scan keyframes for video fields and entity index values for required non-video fields."""
    seg_ids = sorted({seg.segment_id for seg, _ in segment_maxes})
    entities = sorted({
        field.path.split(":")[0] for key, field in fields.items() if key in required and not is_video_field(field)
    })

    def keyframe_task() -> dict[str, _SegmentIndices]:
        # `_fetch_prior_keyframes` only reads each segment's largest target, so one
        # representative per segment reproduces the same per-segment maxima.
        return _fetch_prior_keyframes(
            view=view, index=index, fields=fields, located=segment_maxes, sample_index=sample_index
        )

    entity_segments = [(entity, seg_id) for entity in entities for seg_id in seg_ids]
    segment_tasks = [
        partial(_fetch_entity_index_values, view=view, index=index, entity=entity, segment_id=seg_id)
        for entity, seg_id in entity_segments
    ]

    # `_run_parallel` copies the caller's contextvars into each worker thread, so the
    # collector sees every reader query the scan issues.
    with query_metrics() as metrics:
        keyframes, *segment_values = _run_parallel([keyframe_task, *segment_tasks], max_workers=max_workers)
    _log_scan_metrics(metrics.queries)

    real_by_entity: dict[str, _SegmentIndices] = defaultdict(dict)
    for (entity, seg_id), values in zip(entity_segments, segment_values, strict=True):
        real_by_entity[entity][seg_id] = values
    return _ScanResult(keyframes=keyframes, real_by_entity=dict(real_by_entity))


def _log_scan_metrics(queries: list[QueryMetrics]) -> None:
    """Print the scan's aggregate network cost and attach it to the scan's tracing span."""
    if not queries:
        return
    fetch_bytes = sum(query.fetch_bytes for query in queries)
    max_query_bytes = max(query.fetch_bytes for query in queries)
    direct_requests = sum(query.fetch_direct_requests for query in queries)
    grpc_requests = sum(query.fetch_grpc_requests for query in queries)
    query_dataset_rpcs = len(queries)  # one QueryDataset request per reader query
    print(
        f"Manifest scan fetched {fetch_bytes:,} bytes ({fetch_bytes / 2**20:.1f} MiB) over the wire "
        f"across {len(queries)} queries: {direct_requests:,} direct fetch requests, "
        f"{query_dataset_rpcs + grpc_requests:,} RPC calls "
        f"({query_dataset_rpcs:,} QueryDataset + {grpc_requests:,} chunk fetch); "
        f"heaviest query {max_query_bytes:,} bytes"
    )
    set_current_span_attributes({
        "rerun.dataloader.scan.num_queries": len(queries),
        "rerun.dataloader.scan.network_useful_bytes": fetch_bytes,
        "rerun.dataloader.scan.max_query_fetch_bytes": max_query_bytes,
        "rerun.dataloader.scan.direct_requests": direct_requests,
        "rerun.dataloader.scan.grpc_requests": grpc_requests,
        "rerun.dataloader.scan.query_dataset_attempts": query_dataset_rpcs,
    })


@with_tracing("build_manifest_table._fetch_entity_index_values")
def _fetch_entity_index_values(
    *,
    view: DatasetEntry,
    index: str,
    entity: str,
    segment_id: str,
) -> np.ndarray:
    """Sorted unique observed index values (not queried indices) for one entity path in one segment."""
    column = (
        view.filter_contents([f"{entity}/**"]).filter_segments([segment_id]).reader(index=index).collect_column(index)
    )
    values = column.to_numpy(zero_copy_only=False)
    return np.unique(values.astype(np.int64))


@with_tracing("build_manifest_table._resolve_rows")
def _resolve_rows(
    *,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    scan: _ScanResult,
    required: set[str],
) -> _ResolvedRows:
    """
    Resolve valid samples in canonical order, storing each field's inclusive `[lo, hi]` index range.

    The stored range is exactly the one the reader's decode masks: a field's
    window (`[anchor+w0, anchor+w1]`), a video field's `[prior_keyframe, anchor]`
    GOP, or just `[anchor, anchor]`.

    A `required` field drops the sample when, for any point in its window, the
    nearest real row is missing or older than `max_staleness`. Video uses the
    latest prior keyframe as a conservative proxy for that row and also requires
    a keyframe before the start of its decode range.

    Works one segment at a time: each field's validity and
    `[lo, hi]` range are computed over the segment's whole anchor grid with
    vectorized searchsorted lookups and each
    segment's scan data is released as soon as it is resolved.
    """
    keyframes, real_by_entity = scan.keyframes, scan.real_by_entity
    video = {k: is_video_field(f) for k, f in fields.items()}
    entity_of = {k: f.path.split(":")[0] for k, f in fields.items()}
    deltas = {k: _window_deltas(f, sample_index) for k, f in fields.items()}
    checked = set(fields) & required
    staleness = {k: _staleness_limit(fields[k], sample_index) for k in checked}
    step = sample_index.ns_per_sample or 1

    run_sids: list[str] = []
    run_lengths: list[int] = []
    anchor_chunks: list[np.ndarray] = []
    lo_chunks: dict[str, list[np.ndarray]] = {k: [] for k in fields}
    hi_chunks: dict[str, list[np.ndarray]] = {k: [] for k in fields}
    with tracing_scope("build_manifest_table._resolve_rows.samples"):
        for seg in sample_index.segments:
            sid = seg.segment_id
            anchors = int(seg.index_start) + np.arange(seg.num_samples, dtype=np.int64) * step
            keep = np.ones(seg.num_samples, dtype=bool)
            los: dict[str, np.ndarray] = {}
            his: dict[str, np.ndarray] = {}
            for key in fields:
                field_deltas = deltas[key]
                lo = anchors + int(field_deltas.min())
                hi = anchors + int(field_deltas.max())
                if key in checked:
                    # Video validity uses sparse keyframe timestamps as a conservative
                    # substitute for the complete frame index, avoiding a scan of the
                    # heavy sample component.
                    observed = (
                        keyframes.get(key, {}).get(sid)
                        if video[key]
                        else real_by_entity.get(entity_of[key], {}).get(sid)
                    )
                    keep &= _has_valid_prior(
                        observed,
                        anchors,
                        deltas=field_deltas,
                        max_staleness=staleness[key],
                    )
                if video[key]:
                    # The latest prior keyframe anchors the contiguous decode range.
                    # Required fields must have one or the GOP cannot be decoded.
                    kf, has_kf = _prior_values(keyframes.get(key, {}).get(sid), lo)
                    lo = np.where(has_kf, kf, lo)
                    if key in required:
                        keep &= has_kf
                los[key], his[key] = lo, hi

            if keep.any():
                kept = anchors[keep]
                run_sids.append(sid)
                run_lengths.append(len(kept))
                anchor_chunks.append(kept)
                for key in fields:
                    lo_chunks[key].append(los[key][keep])
                    hi_chunks[key].append(his[key][keep])

            # Release this segment's scan data now that it is resolved.
            for key in fields:
                real_by_entity.get(entity_of[key], {}).pop(sid, None)
                keyframes.get(key, {}).pop(sid, None)

    return _ResolvedRows(
        segment_ids=_dictionary_segment_ids(run_sids, run_lengths),
        anchors=np.concatenate(anchor_chunks) if anchor_chunks else np.empty(0, dtype=np.int64),
        field_ranges={
            k: (
                np.concatenate(lo_chunks[k]) if lo_chunks[k] else np.empty(0, dtype=np.int64),
                np.concatenate(hi_chunks[k]) if hi_chunks[k] else np.empty(0, dtype=np.int64),
            )
            for k in fields
        },
    )


def _dictionary_segment_ids(run_sids: list[str], run_lengths: list[int]) -> pa.DictionaryArray:
    """
    The per-row `segment_id` column as `dictionary<int32 -> string>`, from one `(id, length)` run per segment.

    The dictionary is value-sorted, so the encoding is a pure function of the rows'
    logical content: after the canonical gather, identically-sampled datasets yield
    byte-identical manifests regardless of the order the source enumerated segments.
    """
    dictionary = sorted(run_sids)
    code_of = {sid: code for code, sid in enumerate(dictionary)}
    codes = np.repeat(
        np.array([code_of[sid] for sid in run_sids], dtype=np.int32),
        np.array(run_lengths, dtype=np.int64),
    )
    return pa.DictionaryArray.from_arrays(pa.array(codes), pa.array(dictionary, type=pa.string()))


def _window_deltas(field: Field, sample_index: SampleIndex) -> np.ndarray:
    """
    A field's output offsets relative to its anchor, as int64 index units (`[0]` when unwindowed).

    Mirrors `SampleIndex.offset_index`: seconds scaled to nanoseconds on temporal
    timelines, integral offsets taken as-is on integer timelines.
    """
    if field.window is None:
        return np.zeros(1, dtype=np.int64)
    if sample_index.ns_dtype is not None:
        return np.array([round(float(offset) * 1e9) for offset in field.window], dtype=np.int64)
    for offset in field.window:
        if int(offset) != offset:
            raise ValueError(f"Integer timelines require integral window offsets, got {offset!r}")
    return np.array([int(offset) for offset in field.window], dtype=np.int64)


def _staleness_limit(field: Field, sample_index: SampleIndex) -> int | None:
    """`Field.max_staleness` as an int64 index-unit limit (ns on temporal timelines), or `None`."""
    max_staleness = field.max_staleness
    if max_staleness is None:
        return None
    if sample_index.ns_dtype is not None:
        return round(float(max_staleness) * 1e9)
    if int(max_staleness) != max_staleness:
        raise ValueError(f"Integer timelines require integral max_staleness, got {max_staleness!r}")
    return int(max_staleness)


def _prior_values(sorted_values: np.ndarray | None, targets: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """
    Vectorized `_prior_keyframe`: per target, the largest value `<=` it, and whether one exists.

    Where no prior exists the value slot is meaningless; callers must mask with the second array.
    """
    if sorted_values is None or sorted_values.size == 0:
        return np.zeros_like(targets), np.zeros(targets.shape, dtype=bool)
    pos = np.searchsorted(sorted_values, targets, side="right") - 1
    return sorted_values[np.maximum(pos, 0)], pos >= 0


def _has_valid_prior(
    real: np.ndarray | None,
    anchors: np.ndarray,
    *,
    deltas: np.ndarray,
    max_staleness: int | None,
) -> np.ndarray:
    """Per anchor, whether every window point has a real row at or before it, within `max_staleness`."""
    valid = np.ones(anchors.shape, dtype=bool)
    for delta in deltas.tolist():
        targets = anchors + delta
        prior, exists = _prior_values(real, targets)
        if max_staleness is not None:
            exists &= targets - prior <= max_staleness
        valid &= exists
    return valid


# --------------------------------------------------------------------------------------
# Unroll: the strategy's fetch order + buffer emission order (the cheap part).
# --------------------------------------------------------------------------------------


def _compact_index(seg_ids: pa.Array | pa.ChunkedArray) -> SampleIndex:
    """
    A `SampleIndex` over just the valid samples, one segment per contiguous run of `seg_ids`.

    `seg_ids` are already in canonical (segment, ascending) order, so a run of a
    given id is that segment's valid-sample count. Only `num_samples` (hence
    `segment_offsets`) matters to `_block_order`; the compact index's positions
    map straight back onto the rows of the canonicalized `sample` table.

    Runs are found on the dictionary column's codes with one vectorized diff —
    never on materialized Python strings, which would cost gigabytes at 10^7+ rows.
    """
    if len(seg_ids) == 0:
        return SampleIndex([])
    combined = seg_ids.combine_chunks() if isinstance(seg_ids, pa.ChunkedArray) else seg_ids
    codes = combined.indices.to_numpy(zero_copy_only=False)
    names = combined.dictionary.to_pylist()
    bounds = [0, *(np.flatnonzero(codes[1:] != codes[:-1]) + 1).tolist(), codes.size]
    segments = [
        SegmentMetadata(segment_id=names[int(codes[start])], index_start=0, index_end=0, num_samples=int(end - start))
        for start, end in itertools.pairwise(bounds)
    ]
    return SampleIndex(segments)


def _emit_rank(n: int, buffer: ShuffleBuffer | None, *, seed: int, rank: int, worker: int) -> np.ndarray:
    """
    Emission order for one worker's `n` fetch positions.

    Identity (emit in fetch order) when the strategy defines no buffer;
    otherwise the inverse of `ShuffleBuffer.emit_order`, so `emit_rank[k]` is the
    emission slot of the `k`-th fetched sample. This is the only emission-time
    decorrelation; the fetch order itself stays ascending within each block so
    decode remains monotonic.

    `buffer` is the strategy's own [`emission_buffer`][rerun.experimental.dataloader.ShuffleStrategy.emission_buffer],
    the same object a live run emits through, so a replay can never be baked
    against a differently-configured buffer.
    """
    if buffer is None:
        return np.arange(n, dtype=np.int64)
    rng = np.random.default_rng([seed, rank, worker])
    emit_seq = buffer.emit_order(n, rng=rng)
    emit_rank = np.empty(n, dtype=np.int64)
    emit_rank[emit_seq] = np.arange(n, dtype=np.int64)
    return emit_rank


# --------------------------------------------------------------------------------------
# Table assembly.
# --------------------------------------------------------------------------------------


def _sample_table(rows: _ResolvedRows, field_keys: list[str]) -> pa.Table:
    """
    The per-sample table: `segment_id`, `anchor`, and one `struct<lo, hi>` per field.

    This is everything a manifest carries about a sample independent of the
    schedule — its rows are in resolve order, not yet canonical;
    [`schedule_samples`][] orders them and gathers rows out of it in fetch order.
    """
    # `segment_id` is `dictionary<int32 -> string>`: an int32 code per row plus
    # one id string per segment, ~10x smaller than the per-row string column that
    # otherwise dominates every in-memory copy of the table. The dictionary is
    # value-sorted at build time (see `_dictionary_segment_ids`), so the encoding is
    # determined by the logical content alone and identical manifests stay
    # byte-identical; canonical ordering compares by value via `_segment_sort_key`.
    columns: dict[str, pa.Array] = {
        COL_SEGMENT_ID: rows.segment_ids,
        COL_ANCHOR: pa.array(rows.anchors, type=pa.int64()),
    }
    for key in field_keys:
        lo, hi = rows.field_ranges[key]
        columns[key] = pa.StructArray.from_arrays(
            [pa.array(lo, type=pa.int64()), pa.array(hi, type=pa.int64())],
            names=[RANGE_LO, RANGE_HI],
        )
    return pa.table(columns)


# Columns the schedule owns and regenerates; everything else is a per-sample column.
_SCHEDULE_COLUMNS = frozenset({COL_RANK, COL_WORKER, COL_FETCH_GROUP, COL_EMIT_RANK})


def _canonical_order(sample: pa.Table) -> pa.Array:
    """
    Sort indices that put a per-sample table in canonical `(segment_id, anchor ascending)` order.

    This is the single definition of canonical order both the build and reschedule
    paths obey, so a given `seed` yields the same epoch order regardless of the
    order the source enumerated its segments. Each segment becomes one contiguous
    run and anchors ascend within it, which is all `_compact_index` and the
    monotonic-decode invariant require.

    The `segment_id` column is ordered through per-value ranks, so the order
    follows the id *values* whatever order the dictionary itself is in (a
    parquet round-trip rebuilds dictionaries in appearance order).
    """
    return pc.sort_indices(
        pa.table({COL_SEGMENT_ID: _segment_sort_key(sample[COL_SEGMENT_ID]), COL_ANCHOR: sample[COL_ANCHOR]}),
        sort_keys=[(COL_SEGMENT_ID, "ascending"), (COL_ANCHOR, "ascending")],
    )


def _segment_sort_key(seg_ids: pa.Array | pa.ChunkedArray) -> pa.Array:
    """An int32 rank per row of the dictionary-encoded `seg_ids` that orders like the id values."""
    combined = seg_ids.combine_chunks() if isinstance(seg_ids, pa.ChunkedArray) else seg_ids
    order = pc.sort_indices(combined.dictionary).to_numpy()
    rank = np.empty(order.size, dtype=np.int32)
    rank[order] = np.arange(order.size, dtype=np.int32)
    return pa.array(rank[combined.indices.to_numpy(zero_copy_only=False)])


@with_tracing("schedule_samples")
def schedule_samples(
    sample: pa.Table,
    *,
    strategy: ShuffleStrategy,
    fetch_block_size: int,
    num_ranks: int,
    num_workers_per_rank: int,
    seed: int,
) -> pa.Table:
    """
    Canonicalize `sample`, unroll one epoch across every `(rank, worker)`, and gather the result.

    The single entry point for both building (from a freshly scanned per-sample
    table) and re-shuffling (from a whole manifest): any existing schedule columns
    are dropped so the per-sample columns (`segment_id`, `anchor`, per-field
    `struct<lo, hi>`) are all that's kept. The returned bare data table lays fresh
    schedule columns (`rank`, `worker`, `fetch_group`, `emit_rank`) over those
    samples gathered in fetch order; the caller re-attaches the metadata header.

    `strategy` fixes the fetch order (each block ascending, so decode stays
    monotonic) *and* the emission buffer, if it defines one;
    `num_ranks` / `num_workers_per_rank` split the epoch into per-worker slices.
    """
    sample = sample.select([c for c in sample.column_names if c not in _SCHEDULE_COLUMNS])
    # The canonical sort stays an index array: only the `segment_id` column is
    # gathered here (for `_compact_index`), and the sort is composed with the
    # fetch-order gather below, so the full table is copied once, not twice.
    canonical_order = _canonical_order(sample)
    compact = _compact_index(sample[COL_SEGMENT_ID].take(canonical_order))
    indices, bounds = strategy.epoch_order(compact, fetch_block_size=fetch_block_size, seed=seed)
    buffer = strategy.emission_buffer()

    ranks: list[np.ndarray] = []
    workers: list[np.ndarray] = []
    groups: list[np.ndarray] = []
    emits: list[np.ndarray] = []
    sids: list[np.ndarray] = []
    for rank in range(num_ranks):
        r_idx, r_bounds = _contiguous_shard(indices, bounds, rank=rank, world_size=num_ranks)
        for worker in range(num_workers_per_rank):
            w_idx, w_bounds = _contiguous_shard(r_idx, r_bounds, rank=worker, world_size=num_workers_per_rank)
            blocks = _fetch_blocks(w_idx, w_bounds, fetch_block_size=fetch_block_size)
            if not blocks:
                continue
            fetch_order = np.concatenate(blocks)
            n = int(fetch_order.shape[0])
            ranks.append(np.full(n, rank, dtype=np.int32))
            workers.append(np.full(n, worker, dtype=np.int32))
            groups.append(np.repeat(np.arange(len(blocks), dtype=np.int64), [len(c) for c in blocks]))
            emits.append(_emit_rank(n, buffer, seed=seed, rank=rank, worker=worker))
            sids.append(fetch_order)

    def _concat(parts: list[np.ndarray], dtype: type) -> np.ndarray:
        return np.concatenate(parts) if parts else np.empty(0, dtype=dtype)

    fetch_positions = pa.array(_concat(sids, np.int64), type=pa.int64())
    ordered = sample.take(canonical_order.take(fetch_positions))
    schedule_columns = {
        COL_RANK: pa.array(_concat(ranks, np.int32), type=pa.int32()),
        COL_WORKER: pa.array(_concat(workers, np.int32), type=pa.int32()),
        COL_FETCH_GROUP: pa.array(_concat(groups, np.int64), type=pa.int64()),
        COL_EMIT_RANK: pa.array(_concat(emits, np.int64), type=pa.int64()),
    }
    return pa.table({**schedule_columns, **{name: ordered[name] for name in ordered.column_names}})
