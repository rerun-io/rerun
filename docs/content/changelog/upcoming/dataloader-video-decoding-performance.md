---
title: "Faster video decoding for training"
hidden: true
type: feature
---

### Faster video decoding for training

The experimental PyTorch dataloader now provides three opt-in controls that reduce the cost of decoding overlapping video windows:

- `window_storage="view"` stores each unique decoded frame once per contiguous run and returns views into the shared frame bank.
- `output_format="yuv420p"` skips CPU RGB conversion and keeps compact YUV planes through collation, ready for transfer and conversion on the GPU.
- `thread_count` exposes FFmpeg frame threading for H.264 and H.265 streams.

Because video views share storage, callers must not mutate window values in place before collation.
The existing single-threaded, copied RGB output remains the default.

```python
video_decoder = VideoFrameDecoder(
    codec="h264",
    thread_count=4,
    window_storage="view",
    output_format="yuv420p",
)

loader = DataLoader(dataset, collate_fn=Yuv420Collator(), pin_memory=True)

for batch in loader:
    batch["video"] = batch["video"].to_rgb("cuda", non_blocking=True)
```

Docs: ../howto/train/dataloader.md
