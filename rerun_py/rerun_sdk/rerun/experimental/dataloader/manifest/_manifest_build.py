"""Build a sampling-manifest table by unrolling one epoch of blockwise sampling."""

# Scans the source once to resolve every sample's real decode ranges and drop
# invalid samples, then unrolls one epoch of the blockwise sampling procedure —
# the strategy's fetch order plus the reservoir emission shuffle — into a single
# parquet table. The unroll reuses the exact runtime primitives (`ShuffleStrategy`,
# `_fetch_chunks`, `ShuffleBuffer`) over sample IDs, so build and runtime can
# never diverge, and no decoding happens here.

from __future__ import annotations

import itertools
from collections import defaultdict
from dataclasses import dataclass
from functools import partial
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc

from rerun._tracing import tracing_scope, with_tracing

from .._sample_index import SampleIndex, SegmentMetadata
from .._shuffle import NoShuffle, ShuffleBuffer, ShuffleStrategy, _contiguous_shard, _fetch_chunks
from .._utils import (
    _fetch_prior_keyframes,
    _field_index_range,
    _prior_keyframe,
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
    from .._decoders import ColumnDecoder
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

    segment_ids: list[str]
    anchors: list[int]
    field_ranges: dict[str, list[tuple[int, int]]]  # {field_key: [(lo, hi) per sample]}


@with_tracing("build_manifest_table")
def build_manifest_table(
    source: DataSource,
    index: str,
    fields: dict[str, Field],
    *,
    timeline_sampling: FixedRateSampling | None = None,
    fetch_size: int = 128,
    num_ranks: int = 1,
    num_workers_per_rank: int = 1,
    required_fields: list[str] | None = None,
) -> pa.Table:
    """
    Build a validity-checked, **unshuffled** sampling-manifest table for one epoch.

    Scans the source once to capture each sample's **actual observed** index
    values per field (not the algebraic grid), drops samples whose required
    fields have no usable data, then unrolls one epoch in natural order (see
    [`NoShuffle`][rerun.experimental.dataloader.NoShuffle]) into per-`(rank, worker)`
    `fetch_group` and `emit_rank` columns. Re-order it later with
    [`schedule_samples`][]. Returns the Arrow table (header in the schema
    metadata); persist it via
    [`Manifest.from_arrow`][rerun.experimental.dataloader.Manifest.from_arrow]`.write_parquet`.

    Parameters
    ----------
    source, index, fields, timeline_sampling
        Describe the sample space, exactly as for
        [`RerunIterableDataset`][rerun.experimental.dataloader.RerunIterableDataset].
    fetch_size
        Samples per co-fetch / co-decode block (one `fetch_group`).
    num_ranks, num_workers_per_rank
        DataLoader topology the `(rank, worker)` assignment is frozen for.
    required_fields
        Field keys that must resolve to real data for a sample to be kept;
        defaults to all fields.

    """
    decoders = {k: f.decode for k, f in fields.items()}
    required = set(required_fields) if required_fields is not None else set(fields)

    sample_index = SampleIndex.build(source, index, fields, timeline_sampling=timeline_sampling)

    conn = _WorkerConnection(
        catalog_url=source.dataset.catalog.url,
        dataset_name=source.dataset.name,
        fields=fields,
    )
    view, _ = conn.ensure()

    located = [sample_index.global_to_local(i) for i in range(sample_index.total_samples)]
    scan = _scan(
        view=view,
        index=index,
        fields=fields,
        decoders=decoders,
        sample_index=sample_index,
        located=located,
        required=required,
    )

    rows = _resolve_rows(
        located=located,
        fields=fields,
        decoders=decoders,
        sample_index=sample_index,
        scan=scan,
        required=required,
    )

    # A freshly built manifest is unshuffled: natural fetch order, no emission reservoir.
    strategy = NoShuffle()
    table = schedule_samples(
        _sample_table(rows, list(fields)),
        strategy=strategy,
        fetch_size=fetch_size,
        buffer_size=None,
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
        fetch_size=fetch_size,
        buffer_size=None,
        num_ranks=num_ranks,
        num_workers_per_rank=num_workers_per_rank,
        seed=0,
        shuffle_strategy=strategy.RECIPE_TAG,
    )
    return table.replace_schema_metadata(_metadata_to_arrow(meta))


# --------------------------------------------------------------------------------------
# Scan: resolve the validated sample space (the expensive part).
# --------------------------------------------------------------------------------------


def _keyframe_covered(field: Field, decoder: ColumnDecoder) -> bool:
    """
    Whether a video field's validity is already decided by its prior-keyframe check.

    Such a field needs no real-index scan: a prior keyframe is itself a real row,
    so `_too_far_back`'s existence test can never drop a sample the keyframe check
    keeps. Only holds without a window (staleness would need the nearest real row).
    """
    return is_video_field(field, decoder) and field.window is None and field.max_staleness is None


@with_tracing("build_manifest_table._scan")
def _scan(
    *,
    view: DatasetEntry,
    index: str,
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    located: list[tuple[SegmentMetadata, Any]],
    required: set[str],
) -> _ScanResult:
    """Run the source scans concurrently: prior keyframes (video) and each entity's real index values per segment."""
    seg_ids = sorted({seg.segment_id for seg, _ in located})
    # Only scan entities whose validity still depends on a real-index lookup; a
    # keyframe-covered field's entity is skipped entirely (see `_keyframe_covered`).
    entities = (
        sorted({
            field.path.split(":")[0]
            for key, field in fields.items()
            if key in required and not _keyframe_covered(field, decoders[key])
        })
        if seg_ids
        else []
    )

    def keyframe_task() -> dict[str, _SegmentIndices]:
        return _fetch_prior_keyframes(
            view=view, index=index, fields=fields, decoders=decoders, located=located, sample_index=sample_index
        )

    entity_segments = [(entity, seg_id) for entity in entities for seg_id in seg_ids]
    segment_tasks = [
        partial(_fetch_entity_index_values, view=view, index=index, entity=entity, segment_id=seg_id)
        for entity, seg_id in entity_segments
    ]

    keyframes, *segment_values = _run_parallel([keyframe_task, *segment_tasks], max_workers=_SCAN_MAX_WORKERS)
    real_by_entity: dict[str, _SegmentIndices] = defaultdict(dict)
    for (entity, seg_id), values in zip(entity_segments, segment_values, strict=True):
        real_by_entity[entity][seg_id] = values
    return _ScanResult(keyframes=keyframes, real_by_entity=dict(real_by_entity))


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
    located: list[tuple[SegmentMetadata, Any]],
    fields: dict[str, Field],
    decoders: dict[str, ColumnDecoder],
    sample_index: SampleIndex,
    scan: _ScanResult,
    required: set[str],
) -> _ResolvedRows:
    """
    Resolve valid samples in canonical order, storing each field's inclusive `[lo, hi]` index range.

    The stored range is exactly the one the reader's decode masks: a field's
    window (`[anchor+w0, anchor+w1]`), a video field's `[prior_keyframe, anchor]`
    GOP, or just `[anchor, anchor]`. Storing it means reading a manifest needs no
    window arithmetic and no keyframe scan — the ranges are precomputed here.

    A `required` field drops the sample when, for any point in its window, the
    nearest real row is missing, older than `max_staleness`, or for video it
    has no prior keyframe to decode from.
    """
    keyframes, real_by_entity = scan.keyframes, scan.real_by_entity
    video = {k: is_video_field(f, decoders[k]) for k, f in fields.items()}
    covered = {k: _keyframe_covered(f, decoders[k]) for k, f in fields.items()}
    entity_of = {k: f.path.split(":")[0] for k, f in fields.items()}

    seg_ids: list[str] = []
    anchors: list[int] = []
    field_cols: dict[str, list[tuple[int, int]]] = {k: [] for k in fields}
    with tracing_scope("build_manifest_table._resolve_rows.samples"):
        for seg, idx_val in located:
            iv = int(idx_val)
            ranges: dict[str, tuple[int, int]] = {}
            keep = True
            for key, field in fields.items():
                real = real_by_entity.get(entity_of[key], {}).get(seg.segment_id)
                if (
                    key in required
                    and not covered[key]
                    and _too_far_back(real, iv, field=field, sample_index=sample_index)
                ):
                    keep = False
                    break
                kf = None
                if video[key] and field.window is None:
                    # The prior keyframe may sit arbitrarily far back (exempt from
                    # staleness) but must exist, else the GOP can't be decoded.
                    kf = _prior_keyframe(keyframes.get(key, {}).get(seg.segment_id), iv)
                    if key in required and kf is None:
                        keep = False
                        break
                lo, hi = _field_index_range(iv, field, decoders[key], prior_keyframe=kf) or (iv, iv)
                ranges[key] = (int(lo), int(hi))
            if not keep:
                continue

            seg_ids.append(seg.segment_id)
            anchors.append(iv)
            for key in fields:
                field_cols[key].append(ranges[key])
    return _ResolvedRows(segment_ids=seg_ids, anchors=anchors, field_ranges=field_cols)


def _too_far_back(real: np.ndarray | None, idx_val: int, *, field: Field, sample_index: SampleIndex) -> bool:
    """Whether any point in *field*'s window has no real row at or before it, or one older than `max_staleness`."""
    for g in _grid_timestamps(idx_val, field, sample_index):
        prior = _prior_keyframe(real, g)
        if prior is None:
            return True
        if field.max_staleness is not None and g - prior > field.max_staleness:
            return True
    return False


def _grid_timestamps(idx_val: int, field: Field, sample_index: SampleIndex) -> list[int]:
    """Interpolate between lo and hi for a particular sample to construct a grid of queried timestamps."""
    if field.window is None:
        return [idx_val]
    lo = idx_val + field.window[0]
    hi = idx_val + field.window[1]
    return sorted(int(v) for v in sample_index.indices_in_range(lo, hi))


# --------------------------------------------------------------------------------------
# Unroll: the strategy's fetch order + reservoir emission order (the cheap part).
# --------------------------------------------------------------------------------------


def _compact_index(seg_ids: list[str]) -> SampleIndex:
    """
    A `SampleIndex` over just the valid samples, one segment per contiguous run of `seg_ids`.

    `seg_ids` are already in canonical (segment, ascending) order, so a run of a
    given id is that segment's valid-sample count. Only `num_samples` (hence
    `segment_offsets`) matters to `_block_order`; the compact index's positions
    map straight back onto the rows of the canonicalized `sample` table.
    """
    segments = [
        SegmentMetadata(segment_id=sid, index_start=0, index_end=0, num_samples=sum(1 for _ in run))
        for sid, run in itertools.groupby(seg_ids)
    ]
    return SampleIndex(segments)


def _emit_rank(n: int, buffer_size: int | None, *, seed: int, rank: int, worker: int) -> np.ndarray:
    """
    Emission order for one worker's `n` fetch positions.

    Identity (emit in fetch order) when there is no reservoir; otherwise the
    inverse of
    [`ShuffleBuffer.emit_order`][rerun.experimental.dataloader.ShuffleBuffer.emit_order],
    so `emit_rank[k]` is the emission slot of the `k`-th fetched sample. This is
    the only emission-time decorrelation; the fetch order itself stays ascending
    within each block so decode remains monotonic.
    """
    if buffer_size is None:
        return np.arange(n, dtype=np.int64)
    rng = np.random.default_rng([seed, rank, worker])
    emit_seq = ShuffleBuffer(buffer_size).emit_order(n, rng=rng)
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
    # `segment_id` is a plain string, not dictionary-encoded: `pq.write_table`
    # dictionary-encodes it on disk for free, and keeping it a string in memory
    # makes canonicalization and `Table.equals` compare by value (no dictionary-index
    # ambiguity), so identical manifests stay byte-identical with no extra work.
    columns: dict[str, pa.Array] = {
        COL_SEGMENT_ID: pa.array(rows.segment_ids, type=pa.string()),
        COL_ANCHOR: pa.array(rows.anchors, type=pa.int64()),
    }
    for key in field_keys:
        columns[key] = _range_struct(rows.field_ranges[key])
    return pa.table(columns)


def _range_struct(rows: list[tuple[int, int]]) -> pa.Array:
    """Build an `int64 struct<lo, hi>` column from per-sample inclusive `[lo, hi]` ns / step ranges."""
    raw_lo, raw_hi = zip(*rows, strict=True) if rows else ((), ())
    lo = pa.array(raw_lo, type=pa.int64())
    hi = pa.array(raw_hi, type=pa.int64())
    return pa.StructArray.from_arrays([lo, hi], names=[RANGE_LO, RANGE_HI])


# Columns the schedule owns and regenerates; everything else is a per-sample column.
_SCHEDULE_COLUMNS = frozenset({COL_RANK, COL_WORKER, COL_FETCH_GROUP, COL_EMIT_RANK})


def _canonicalize(sample: pa.Table) -> pa.Table:
    """
    Order a per-sample table into canonical `(segment_id, anchor ascending)` order.

    This is the single definition of canonical order both the build and reschedule
    paths obey, so a given `seed` yields the same epoch order regardless of the
    order the source enumerated its segments. Each segment becomes one contiguous
    run and anchors ascend within it, which is all `_compact_index` and the
    monotonic-decode invariant require.
    """
    order = pc.sort_indices(
        pa.table({COL_SEGMENT_ID: sample[COL_SEGMENT_ID], COL_ANCHOR: sample[COL_ANCHOR]}),
        sort_keys=[(COL_SEGMENT_ID, "ascending"), (COL_ANCHOR, "ascending")],
    )
    return sample.take(order)


@with_tracing("schedule_samples")
def schedule_samples(
    sample: pa.Table,
    *,
    strategy: ShuffleStrategy,
    fetch_size: int,
    buffer_size: int | None,
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
    monotonic) and `num_ranks` / `num_workers_per_rank` split the epoch into
    per-worker slices.
    """
    sample = _canonicalize(sample.select([c for c in sample.column_names if c not in _SCHEDULE_COLUMNS]))
    compact = _compact_index(sample[COL_SEGMENT_ID].to_pylist())
    indices, bounds = strategy.epoch_order(compact, fetch_size=fetch_size, seed=seed)

    ranks: list[np.ndarray] = []
    workers: list[np.ndarray] = []
    groups: list[np.ndarray] = []
    emits: list[np.ndarray] = []
    sids: list[np.ndarray] = []
    for rank in range(num_ranks):
        r_idx, r_bounds = _contiguous_shard(indices, bounds, rank=rank, world_size=num_ranks)
        for worker in range(num_workers_per_rank):
            w_idx, w_bounds = _contiguous_shard(r_idx, r_bounds, rank=worker, world_size=num_workers_per_rank)
            chunks = _fetch_chunks(w_idx, w_bounds, fetch_size=fetch_size)
            if not chunks:
                continue
            fetch_order = np.concatenate(chunks)
            n = int(fetch_order.shape[0])
            ranks.append(np.full(n, rank, dtype=np.int32))
            workers.append(np.full(n, worker, dtype=np.int32))
            groups.append(np.repeat(np.arange(len(chunks), dtype=np.int64), [len(c) for c in chunks]))
            emits.append(_emit_rank(n, buffer_size, seed=seed, rank=rank, worker=worker))
            sids.append(fetch_order)

    def _concat(parts: list[np.ndarray], dtype: type) -> np.ndarray:
        return np.concatenate(parts) if parts else np.empty(0, dtype=dtype)

    ordered = sample.take(pa.array(_concat(sids, np.int64), type=pa.int64()))
    schedule_columns = {
        COL_RANK: pa.array(_concat(ranks, np.int32), type=pa.int32()),
        COL_WORKER: pa.array(_concat(workers, np.int32), type=pa.int32()),
        COL_FETCH_GROUP: pa.array(_concat(groups, np.int64), type=pa.int64()),
        COL_EMIT_RANK: pa.array(_concat(emits, np.int64), type=pa.int64()),
    }
    return pa.table({**schedule_columns, **{name: ordered[name] for name in ordered.column_names}})
