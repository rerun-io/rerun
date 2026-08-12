"""Build a sampling-manifest table by unrolling one epoch of blockwise sampling."""

# Scans the source once to resolve every sample's real decode ranges and drop
# invalid samples, then unrolls one epoch of the blockwise sampling procedure —
# the strategy's fetch order plus the buffer emission shuffle — into a single
# parquet table. The unroll reuses the exact runtime primitives (`ShuffleStrategy`,
# `_fetch_chunks`, `ShuffleBuffer`) over sample IDs, so build and runtime can
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

    segment_ids: pa.Array  # string
    anchors: np.ndarray  # int64
    field_ranges: dict[str, tuple[np.ndarray, np.ndarray]]  # {field_key: (lo, hi) int64 arrays}


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
    scan_max_workers: int | None = None,
) -> pa.Table:
    """
    Build a validity-checked, **unshuffled** sampling-manifest table for one epoch.

    Scans the source once to capture each sample's **actual observed** index
    values per field (not the algebraic grid), drops samples whose required
    fields have no usable data, then unrolls one epoch in natural order (see
    [`NoShuffle`][rerun.experimental.dataloader.NoShuffle]) into per-`(rank, worker)`
    `fetch_group` and `emit_rank` columns. Re-order it later with
    [`schedule_samples`][]. Returns the Arrow table (header in the schema
    metadata); [`Manifest.generate`][rerun.experimental.dataloader.Manifest.generate] wraps it into a
    manifest, which is persisted with
    [`Manifest.write_parquet`][rerun.experimental.dataloader.Manifest.write_parquet].

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
    scan_max_workers
        Max concurrent scan queries against the server. Higher values speed up
        the (dominant) scan of a large dataset at the cost of more server load
        and memory; defaults to `8`.

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
        decoders=decoders,
        sample_index=sample_index,
        segment_maxes=segment_maxes,
        required=required,
        max_workers=scan_max_workers if scan_max_workers is not None else _SCAN_MAX_WORKERS,
    )

    rows = _resolve_rows(
        fields=fields,
        decoders=decoders,
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
        fetch_size=fetch_size,
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
    segment_maxes: list[tuple[SegmentMetadata, int]],
    required: set[str],
    max_workers: int,
) -> _ScanResult:
    """Run the source scans concurrently: prior keyframes (video) and each entity's real index values per segment."""
    seg_ids = sorted({seg.segment_id for seg, _ in segment_maxes})
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
        # `_fetch_prior_keyframes` only reads each segment's largest target, so one
        # representative per segment reproduces the same per-segment maxima.
        return _fetch_prior_keyframes(
            view=view, index=index, fields=fields, decoders=decoders, located=segment_maxes, sample_index=sample_index
        )

    entity_segments = [(entity, seg_id) for entity in entities for seg_id in seg_ids]
    segment_tasks = [
        partial(_fetch_entity_index_values, view=view, index=index, entity=entity, segment_id=seg_id)
        for entity, seg_id in entity_segments
    ]

    keyframes, *segment_values = _run_parallel([keyframe_task, *segment_tasks], max_workers=max_workers)
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

    Works one segment at a time, packing each segment's kept samples straight into
    numpy and releasing that segment's scan data, so the whole sample space is never
    resident as Python objects.
    """
    keyframes, real_by_entity = scan.keyframes, scan.real_by_entity
    video = {k: is_video_field(f, decoders[k]) for k, f in fields.items()}
    covered = {k: _keyframe_covered(f, decoders[k]) for k, f in fields.items()}
    entity_of = {k: f.path.split(":")[0] for k, f in fields.items()}
    step = sample_index.ns_per_sample or 1

    seg_id_chunks: list[pa.Array] = []
    anchor_chunks: list[np.ndarray] = []
    lo_chunks: dict[str, list[np.ndarray]] = {k: [] for k in fields}
    hi_chunks: dict[str, list[np.ndarray]] = {k: [] for k in fields}
    with tracing_scope("build_manifest_table._resolve_rows.samples"):
        for seg in sample_index.segments:
            sid = seg.segment_id
            # This segment's scan data is shared across all its samples.
            observed_index_values_by_field = {k: real_by_entity.get(entity_of[k], {}).get(sid) for k in fields}
            keyframe_positions_by_field = {k: keyframes.get(k, {}).get(sid) for k in fields}

            anchors: list[int] = []
            los: dict[str, list[int]] = {k: [] for k in fields}
            his: dict[str, list[int]] = {k: [] for k in fields}
            for index_value in (int(seg.index_start) + np.arange(seg.num_samples, dtype=np.int64) * step).tolist():
                ranges: dict[str, tuple[int, int]] = {}
                keep = True
                for key, field in fields.items():
                    if (
                        key in required
                        and not covered[key]
                        and _too_far_back(
                            observed_index_values_by_field[key], index_value, field=field, sample_index=sample_index
                        )
                    ):
                        keep = False
                        break
                    kf = None
                    if video[key] and field.window is None:
                        # The prior keyframe may sit arbitrarily far back (exempt from
                        # staleness) but must exist, else the GOP can't be decoded.
                        kf = _prior_keyframe(keyframe_positions_by_field[key], index_value)
                        if key in required and kf is None:
                            keep = False
                            break
                    lo, hi = _field_index_range(index_value, field, decoders[key], prior_keyframe=kf) or (
                        index_value,
                        index_value,
                    )
                    ranges[key] = (int(lo), int(hi))
                if not keep:
                    continue
                anchors.append(index_value)
                for key in fields:
                    los[key].append(ranges[key][0])
                    his[key].append(ranges[key][1])

            if anchors:
                seg_id_chunks.append(pa.array([sid] * len(anchors), type=pa.string()))
                anchor_chunks.append(np.asarray(anchors, dtype=np.int64))
                for key in fields:
                    lo_chunks[key].append(np.asarray(los[key], dtype=np.int64))
                    hi_chunks[key].append(np.asarray(his[key], dtype=np.int64))

            # Release this segment's scan data now that it is resolved.
            for key in fields:
                real_by_entity.get(entity_of[key], {}).pop(sid, None)
                keyframes.get(key, {}).pop(sid, None)

    return _ResolvedRows(
        segment_ids=pa.concat_arrays(seg_id_chunks) if seg_id_chunks else pa.array([], type=pa.string()),
        anchors=np.concatenate(anchor_chunks) if anchor_chunks else np.empty(0, dtype=np.int64),
        field_ranges={
            k: (
                np.concatenate(lo_chunks[k]) if lo_chunks[k] else np.empty(0, dtype=np.int64),
                np.concatenate(hi_chunks[k]) if hi_chunks[k] else np.empty(0, dtype=np.int64),
            )
            for k in fields
        },
    )


def _too_far_back(real: np.ndarray | None, index_value: int, *, field: Field, sample_index: SampleIndex) -> bool:
    """Whether any point in *field*'s window has no real row at or before it, or one older than `max_staleness`."""
    for g in _grid_timestamps(index_value, field, sample_index):
        prior = _prior_keyframe(real, g)
        if prior is None:
            return True
        if field.max_staleness is not None and g - prior > field.max_staleness:
            return True
    return False


def _grid_timestamps(index_value: int, field: Field, sample_index: SampleIndex) -> list[int]:
    """Interpolate between lo and hi for a particular sample to construct a grid of queried timestamps."""
    if field.window is None:
        return [index_value]
    lo = index_value + field.window[0]
    hi = index_value + field.window[1]
    return sorted(int(v) for v in sample_index.indices_in_range(lo, hi))


# --------------------------------------------------------------------------------------
# Unroll: the strategy's fetch order + buffer emission order (the cheap part).
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
    # `segment_id` is a plain string, not dictionary-encoded: `pq.write_table`
    # dictionary-encodes it on disk for free, and keeping it a string in memory
    # makes canonicalization and `Table.equals` compare by value (no dictionary-index
    # ambiguity), so identical manifests stay byte-identical with no extra work.
    # TODO(guillaume): dictionary-encoding `segment_id` at *read* time (one UUID per
    # segment instead of one per row) is a potential optimization for the manifest's
    # RAM footprint — it's the dominant column when the whole table is resident. The
    # build-time byte-identity concern above only applies here, not on the read path.
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
    sample = _canonicalize(sample.select([c for c in sample.column_names if c not in _SCHEDULE_COLUMNS]))
    compact = _compact_index(sample[COL_SEGMENT_ID].to_pylist())
    indices, bounds = strategy.epoch_order(compact, fetch_size=fetch_size, seed=seed)
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
            chunks = _fetch_chunks(w_idx, w_bounds, fetch_size=fetch_size)
            if not chunks:
                continue
            fetch_order = np.concatenate(chunks)
            n = int(fetch_order.shape[0])
            ranks.append(np.full(n, rank, dtype=np.int32))
            workers.append(np.full(n, worker, dtype=np.int32))
            groups.append(np.repeat(np.arange(len(chunks), dtype=np.int64), [len(c) for c in chunks]))
            emits.append(_emit_rank(n, buffer, seed=seed, rank=rank, worker=worker))
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
