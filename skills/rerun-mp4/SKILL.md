---
name: rerun-mp4
description: Ingest .mp4 video into Rerun chunk streams with rerun.experimental.Mp4Reader. Read when converting video into a VideoStream, choosing stream vs asset mode, transcoding (B-frames, output codec, GOP size) through FFmpeg, or aligning video PTS onto a recording's wall-clock timeline. Builds on rerun-chunk-processing (stream mechanics) and rerun-data-model (where the video belongs in the recording).
user_invocable: true
allowed-tools: Read, Grep, Bash, WebFetch
---

# Rerun mp4 ingestion

`Mp4Reader` turns one `.mp4` file into a lazy chunk stream on **one entity**:
compressed video samples, no decode to pixels, nothing re-encoded unless it has
to be. Everything is configured on the constructor; `stream()` takes no
arguments. Stream mechanics after `.stream()` (filter, map, merge, collect,
write) are in `rerun-chunk-processing`.

## The API

```python
from rerun.experimental import Mp4Reader, Mp4TranscodeOptions

reader = Mp4Reader(video_path, entity_path="/camera/front")  # mode="stream" by default
stream = reader.stream()  # lazy: nothing is decoded yet
```

Every parameter after `path` is keyword-only. One reader handles one file — for
several cameras, build one reader per file and `LazyChunkStream.merge(...)` them
(see below).

## Two modes

| `mode`               | emits                                                   | when                                                              |
| -------------------- | ------------------------------------------------------- | ----------------------------------------------------------------- |
| `"stream"` (default) | a codec chunk, per-GOP sample chunks, a keyframe marker | almost always — every frame is time-indexed and queryable         |
| `"asset"`            | the whole file as one blob, plus a frame index          | the codec cannot be a `VideoStream`, or the source starts mid-GOP |

Reach for stream mode; the next section covers exactly what it emits. Asset mode
copies the entire file into the recording as one blob, so nothing downstream can
look at a single frame without the whole asset. It is the fallback for codecs a
`VideoStream` cannot carry (`mp4v`, image-sequence mp4), and it is what `rerun
video.mp4` and the built-in file importer use.

## What stream mode emits

For `tests/assets/video/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4` (30 frames,
one GOP) with `entity_path="/camera/front"`, the whole output is three chunks:

```
[0] entity=/camera/front static=True rows=1 timelines=[] cols=['VideoStream:codec']
[1] entity=/camera/front static=False rows=30 timelines=['video'] cols=['VideoStream:sample']
[2] entity=/camera/front static=False rows=1 timelines=['video'] cols=['VideoStream:is_keyframe']
```

- **One static chunk, one row**, holding `VideoStream:codec`.
  It carries **no timeline** — code that walks the stream must
  handle that (`if chunk.is_static: continue`).
- **One temporal chunk per GOP**: a keyframe plus every sample that depends on
  it, up to (not including) the next keyframe. Samples only — the keyframe flags
  are not a column here.
- **One trailing `is_keyframe` marker chunk**, holding a *sparse* `True` row at
  each keyframe's time and nothing else. Keeping it out of the sample chunks is
  what lets a keyframe-only query skip the sample payload, and what
  `collect(optimize=…)` accepts as canonical. It is list-per-row, so a row reads
  back as `[True]`, not `True`.
- **Timeline `video`**, `duration[ns]`, values are the mp4 PTS from the start of
  the video.
- **Every chunk on the same entity.** `entity_path=None` (the default) derives
  it from the file's *absolute* path, so `foo/video.mp4` run from `/data`
  becomes `/data/foo/video.mp4`. Pass `entity_path` explicitly in any real
  pipeline.

A short clip is often a single GOP, so "three chunks total" is the normal shape —
not a sign that something was dropped. `chunk_by_gop=False` emits one Rerun
chunk per sample instead (the marker chunk is unaffected). That is a debugging
shape — one chunk per frame is a poor storage layout, and it also makes a
following optimize pass **6× more expensive** (measured below) — so leave the
default alone unless you are inspecting individual samples.

Distinguish the three by their columns, not by position or `is_static`: the codec
chunk is the static one, sample chunks carry `VideoStream:sample`, and the marker
chunk carries only `VideoStream:is_keyframe`.

Supported stream-mode codecs are exactly the five `VideoCodec` values: H264,
H265, AV1, VP8, VP9.

## Transcoding through FFmpeg

`VideoStream` cannot yet model DTS != PTS, so an H.264/H.265 source with
container-level B-frame reordering **cannot be emitted directly**. The reader
handles that itself: it re-encodes through FFmpeg with `-bf 0`, streams the
result back as a fragmented mp4, and turns each fragment into one GOP chunk, so
only one GOP is resident at a time. This is automatic and invisible in the API —
the output has the same shape as a clean source. It does need an `ffmpeg`
executable. Asset mode is unaffected by B-frames.

`Mp4TranscodeOptions` (stream mode only) additionally *requests* a transcode:

| field             | effect                                                                     |
| ----------------- | -------------------------------------------------------------------------- |
| `output_codec`    | re-encode to another `VideoCodec`; the emitted `VideoStream:codec` follows |
| `gop_size`        | force a keyframe every N frames — the knob for seek cost in the viewer     |
| `try_gpu`         | best-effort hardware encode, NVENC / VideoToolbox only                     |
| `ffmpeg_override` | use this `ffmpeg` instead of the one on `PATH`                             |

Requesting the `output_codec` the source already uses is a **no-op**: it stays on
the direct, no-FFmpeg path. With `chunk_by_gop=True`, `gop_size=N` makes every
GOP chunk but the last hold exactly N samples — a 30-frame clip at `gop_size=10`
gives 10, 10, 10. `try_gpu` realistically covers H264/H265 plus AV1 on newer
NVIDIA; VP8/VP9 always fall back to software, and it does nothing unless a
transcode is already happening.

```python
Mp4Reader(
    video_path,
    entity_path="/camera/front",
    # ~1s GOPs on a 60fps source, so seeking in the viewer stays snappy.
    transcode=Mp4TranscodeOptions(gop_size=64),
).stream()
```

## Timelines: mp4 PTS is not your recording's clock

The emitted times are always PTS — elapsed nanoseconds from the start of *that
video*. In a multi-sensor recording, that is almost never the timeline you want
to align on.

- `timeline_name="real_time"` renames the timeline.
- `timeline_type="timestamp"` only **retypes** the same PTS values as
  nanoseconds since the Unix epoch. The reader does not shift them, so on its
  own it renders the video near 1970. It is meaningful only paired with a retag
  step.
- **Retag with `stream.map(...)`.** Samples arrive in presentation order
  (B-frames are already stripped), so a running cursor maps sample `i` to
  `capture_times_ns[i]` — but the cursor must skip the keyframe marker chunk,
  which holds no samples. Map that one through the PTS the samples were already
  assigned:

```python
SAMPLE_COL = "VideoStream:sample"


def _reindex_to_capture_times(stream, capture_times_ns, timeline_name):
    cursor = 0
    pts_to_time = {}

    def _retag(chunk):
        nonlocal cursor
        if chunk.is_static:  # the codec chunk carries no timeline
            return chunk
        batch = chunk.to_record_batch()
        col_index = batch.schema.get_field_index(timeline_name)
        old_field = batch.schema.field(col_index)
        old_pts = np.asarray(batch.column(col_index).cast(pa.int64()))

        if SAMPLE_COL in batch.schema.names:
            # Clamp in case the decoder yields a slightly different frame count.
            indices = np.clip(np.arange(cursor, cursor + chunk.num_rows), 0, len(capture_times_ns) - 1)
            cursor += chunk.num_rows
            new_times_ns = capture_times_ns[indices]
            pts_to_time.update(zip(old_pts.tolist(), new_times_ns.tolist()))
        else:
            # The sparse keyframe marker, emitted after every sample chunk.
            new_times_ns = np.array([pts_to_time[pts] for pts in old_pts.tolist()], dtype=np.int64)

        times = pa.array(new_times_ns.astype("datetime64[ns]"))
        # `metadata=` is load-bearing — see gotcha 3.
        new_field = pa.field(old_field.name, times.type, nullable=old_field.nullable, metadata=old_field.metadata)
        return Chunk.from_record_batch(batch.set_column(col_index, new_field, times))[0]

    return stream.map(_retag)
```

The cursor makes this order-dependent, so keep the retag on the single reader's
stream, before any merge. This is the DROID loader's pattern — see the
references.

Any `map`/`flat_map` that assumes "every non-static chunk is samples, in order"
has this same bug: it advances over the marker chunk and stamps it with whatever
time comes next, silently moving the keyframe markers off their samples.

## Several cameras into one recording

One reader per file, distinct entity paths, then merge:

```python
streams = [
    Mp4Reader(path, entity_path=f"/camera/{name}", timeline_name="real_time", timeline_type="timestamp").stream()
    for name, path in cameras.items()
]
(
    LazyChunkStream
    .merge(*streams)
    .collect(optimize=OptimizationProfile.OBJECT_STORE)
    .write_rrd(out_path, application_id=app_id, recording_id=segment_id)
)
```

`OBJECT_STORE` GOP-rebatches the video and preserves the reader's keyframe
marker as-is; no `fix_keyframe=True` is needed. If you *do* see
`skipping GoP rebatching … is_keyframe data is incorrect`, something upstream
rewrote the marker (see gotcha 4) — `fix_keyframe=True` re-derives it from the
encoded samples as an escape hatch.

## How the reader's chunks relate to optimize's

The reader and `collect(optimize=…)` agree on **where GOPs start** but not on
**how many GOPs share a chunk**, and that is by design:

- The reader emits the **finest GOP-aligned partition**: exactly one chunk per
  GOP. It cannot do better, because it does not know which profile the data is
  headed for.
- Optimize then applies the size policy, **merging consecutive GOPs** up to the
  profile's `max_bytes` / `max_rows`. It never splits a GOP across chunks, so
  every boundary it keeps is one the reader already produced.

So optimize's partition is a pure *coarsening* of the reader's: same samples in
the same order, every optimized chunk a run of whole reader GOPs, keyframe marker
untouched. Where each GOP already sits near the profile's budget the two come out
identical (a 30-frame clip at `gop_size=10` is 3 chunks either way). Where GOPs
are small they diverge sharply — a 12-GOP clip becomes **1** chunk under
`OBJECT_STORE` and **4** under `LIVE`.

The practical consequence: **do not pre-merge or re-chunk the reader's output**
to "help" optimize. Hand it the per-GOP stream and let the profile decide. And if
you skip optimize entirely (writing straight through `send_chunks`, as the DROID
loader does), you are storing the finest partition — correct, but more chunks
than object storage wants.

### The optimize pass is cheap on GOP-chunked input

Running optimize over the reader's output is not redundant work worth avoiding.
On an 89 MB / 1800-frame / 60-GOP H.264 file:

|                               | `collect()` | `collect(optimize=OBJECT_STORE)` | delta               |
| ----------------------------- | ----------- | -------------------------------- | ------------------- |
| `chunk_by_gop=True` (default) | 11.1 ms     | 16.3 ms                          | **+5.2 ms** (1.5×)  |
| `chunk_by_gop=False`          | 14.2 ms     | 48.6 ms                          | **+34.4 ms** (3.4×) |

Neither half of the pass is expensive on GOP-aligned input:

- **Detecting GOP starts is header-only.** `build_sample_index` runs
  `detect_gop_start` on every sample, but that reads a few bytes of each sample's
  header — it never decodes. The whole optimize delta above (5.2 ms) is smaller
  than a Python loop calling the same detector over the same 1800 samples
  (6.0 ms), which is mostly FFI overhead.
- **Rebuilding the chunks copies nothing.** `chunk_from_gop` calls
  `taken(0..n)`, and `re_arrow_util::take_array` returns the array **uncopied**
  when the indices are consecutive from zero over the whole array. One chunk per
  GOP means that is always the case, so the multi-MB sample buffers are never
  duplicated — hence 89 MB "rebuilt" in 5 ms.

That fast path is exactly what `chunk_by_gop=False` gives up: each GOP then spans
30 source chunks, so `chunk_from_gop` has to `concat_and_sort` them and the sample
bytes really are copied. `LIVE` also costs more than `OBJECT_STORE` (+13.2 ms vs
+5.2 ms on the same file), because its much smaller chunk budget makes it attempt
compaction it cannot complete — a GOP is never split.

Retag each camera *before* the merge — the retag above walks chunks with a
cursor, so it must see one video's chunks in order.

## Gotchas

1. **Errors surface on the first pull, not at construction.** The constructor
   only checks that the file exists and validates its arguments; `stream()`
   builds a lazy pipeline and also succeeds. Codec support, keyframe layout, and
   FFmpeg availability are checked when the stream runs, so wrap the
   *consumption* (`to_chunks()`, `send_chunks`, iteration), not the
   `Mp4Reader(…)` call.
2. **A codec outside the five `VideoCodec` values cannot be a `VideoStream`.**
   `mp4v` (MPEG-4 Part 2) is the one you will actually meet in robot datasets;
   it raises `RuntimeError: MP4 error: MP4 demux: Video track uses unsupported
codec "mp4v"`. Asset mode does accept the file, but only partly: the blob
   chunk is emitted and the `VideoFrameReference` index chunk is **skipped with
   a warning**, because the frame timestamps cannot be read either — so you get
   the bytes and no timeline. Skipping that camera and recording the fact as a
   recording property (what the DROID loader does) is usually better than a
   timeline-less blob, and either beats failing a whole episode over one camera.
3. **A `map`/`flat_map` that rewrites the time column must carry the field
   metadata over.** `pa.field(...)` without `metadata=old_field.metadata` drops
   `rerun:kind: 'index'`, and the rebuilt chunk silently becomes **static** —
   no error, no timeline, and the samples land outside time entirely.
4. **The trailing marker chunk is temporal but holds no samples**, so any
   per-chunk logic keyed on `not chunk.is_static` will process it as if it were
   samples. Key on the `VideoStream:sample` column instead. See the retag above.
5. **Asset mode is capped at ~2 GiB** by Arrow's i32 offsets, and duplicates the
   file's bytes into the RRD.
6. `mode="asset"` rejects both `chunk_by_gop=False` and `transcode=` with
   `ValueError` — those are stream-mode-only knobs.
7. `timeline_type="timestamp"` on its own shifts nothing. Without a retag, the
   video sits at the epoch.
8. **The reader emits compressed samples, never pixels.** Thumbnails, CLIP
   embeddings, or anything else needing RGB have to decode the file separately
   (OpenCV, PyAV); the `VideoStream` chunks cannot supply them.
9. Every `stream()` call re-decodes the file from scratch, and every terminal
   call re-runs the pipeline. `collect()` once when you need more than one pass.
10. Handle the static codec chunk explicitly in any `map`/`flat_map` — it has no
    timeline and no samples, and blind indexing into a time column will fail on
    it.
11. **Samples before the first keyframe** are rejected in stream mode (a decoder
    cannot start mid-GOP); use asset mode for such a file.

## References

- Canonical worked examples: `rerun_py/tests/integration/test_mp4_reader.py`
  (both modes, `chunk_by_gop`, entity paths, `timeline_type`, transcode
  transforms, and the error cases).
- Rust core: `crates/store/re_mp4_reader/` (`stream.rs` for the GOP/transcode
  path, `asset.rs` for the blob+index path), with
  `crates/store/re_mp4_reader/tests/stream.rs` covering codec pairs and GOP
  spacing.
- `rerun-chunk-processing` (stream/lens mechanics), `rerun-data-model` (where
  video, calibration, and thumbnails belong in the recording).
