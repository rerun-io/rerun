---
title: "Faster manifest generation for video datasets"
hidden: true
type: feature
---

### Faster manifest generation for video datasets

Manifest generation for the experimental PyTorch dataloader now uses sparse `VideoStream:is_keyframe` timestamps to validate compressed-video fields and anchor their decode ranges.
It no longer scans `VideoStream:sample` timestamps, avoiding the transfer of expensive encoded video data while building a manifest.
For compressed video, `max_staleness` is conservatively measured from the latest prior keyframe, so a sample may be omitted even when a fresher non-keyframe exists.

Docs: ../howto/train/dataloader.md
