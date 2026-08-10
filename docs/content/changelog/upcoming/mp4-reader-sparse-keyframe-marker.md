---
title: "`Mp4Reader` emits `is_keyframe` as a sparse marker chunk"
hidden: true
type: breaking
---

### `Mp4Reader` emits `is_keyframe` as a sparse marker chunk

Stream mode used to put `VideoStream:is_keyframe` on every sample chunk, one `true`/`false` row per sample.
It now emits a single trailing chunk holding only `is_keyframe`, with one `true` row per keyframe:

```
# 0.35
[0] static -> VideoStream:codec
[1] GOP 0 -> VideoStream:is_keyframe, VideoStream:sample
[2] GOP 1 -> VideoStream:is_keyframe, VideoStream:sample

# 0.36
[0] static -> VideoStream:codec
[1] GOP 0 -> VideoStream:sample
[2] GOP 1 -> VideoStream:sample
[3] marker -> VideoStream:is_keyframe (only `true` rows)
```

GOP rebatching rejects `is_keyframe=false` rows, so the old dense column made `collect(optimize=…)` skip video rebatching entirely unless you passed `fix_keyframe=True`.
It now works with no extra flag, and the marker is kept verbatim instead of rebuilt.
A keyframe-only query (such as the dataloader's frame anchor) can also skip the sample column now that the two no longer share a chunk.

Code that post-processes the reader's stream has to tell the marker chunk apart from the sample chunks.
It is temporal but holds no samples, so logic keyed on `not chunk.is_static` will treat it as samples.
Key on the `VideoStream:sample` column instead:

```python
is_sample_chunk = "VideoStream:sample" in chunk.to_record_batch().schema.names
```

This bites hardest in a `map` that rewrites the time column (e.g. retagging mp4 PTS onto a wall-clock timeline): a positional cursor advances over the marker chunk and stamps it with the wrong time.
