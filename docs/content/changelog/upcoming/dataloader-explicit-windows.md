---
title: "Experimental dataloader windows are explicit and decoder-independent"
hidden: true
type: breaking
---

### Experimental dataloader Windows are explicit and decoder-independent

`Field.window` now specifies the exact offsets to return relative to each sample instead of an inclusive `(start, end)` range.
Windows work with every built-in decoder, including numeric values, Arrow data, images, and compressed video.
Video windows fetch the intermediate GOP frames required for decoding while returning only the explicitly requested frames.

Integer timelines use integral index-step offsets.
Timestamp and duration timelines use seconds, which are converted to nanoseconds internally; `max_staleness` now follows the same convention.

For example, migrate an integer window as follows:

```python
# Before: every index from -2 through 0.
Field(path, decode=decoder, window=(-2, 0))

# After: the exact same three index offsets.
Field(path, decode=decoder, window=(-2, -1, 0))
```

For a timestamp timeline sampled at 10 Hz, migrate nanosecond values to explicit seconds:

```python
# Before: the inclusive 500 ms history range, with staleness in nanoseconds.
Field(path, decode=decoder, window=(-500_000_000, 0), max_staleness=500_000_000)

# After: six explicit samples, with both values expressed in seconds.
Field(
    path,
    decode=decoder,
    window=tuple(step / 10.0 for step in range(-5, 1)),
    max_staleness=0.5,
)
```

Compressed-video fields now require their sibling `VideoStream:is_keyframe` component so decode ranges can reliably begin at a keyframe.
Custom decoders should use `DecodeRequest.decode_row_indices` and `DecodeRequest.output_row_indices`; the decoder-owned `context_range` hook has been removed because fetch requirements are now resolved by the field pipeline.

Docs: ../howto/train/dataloader.md
