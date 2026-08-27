---
title: "Keyframe markers are kept out of the video chunks"
hidden: true
type: feature
---

### Keyframe markers are kept out of the video chunks

`VideoStream:is_keyframe` now always ends up in a chunk of its own, so a reader can scan a video's keyframes without downloading a single video sample.
It used to ride along in the sample chunks unless GoP rebatching happened to move it.

This is the first use of a new type-definition attribute, `#[rerun(own_chunk)]`, which marks a component as one that never shares a chunk with another component.

The split runs wherever chunks are optimized, and it is not optional — unlike the thick/thin split, no setting turns it off:

* `rerun rrd optimize`
* `rerun.experimental.OptimizationProfile`, and the Python chunk-processing pipeline that takes one
* `ChunkStore::compacted` and `ChunkStore::finalize_compaction` in Rust

The chunk store also refuses to merge such a chunk back together with anything else, so a recording keeps the layout once it has it.
