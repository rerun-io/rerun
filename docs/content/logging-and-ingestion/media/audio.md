---
title: Audio
order: 200
description: How to log audio with AssetAudio, including raw PCM samples
---

Rerun plays back audio files logged with the [`AssetAudio`](../../reference/types/archetypes/asset_audio.md) archetype.
See its reference page for the supported formats, how playback lines up with time, and an example.

## Raw PCM samples

There is [no archetype for raw PCM samples yet](../../development/roadmap.md#missing-features).
Instead, encode the samples as a WAV file in memory and log that.
For live audio, do this for short segments, and log each segment at the time it starts:

snippet: howto/pcm_audio

To plot the raw signal instead of playing it, log the samples as [`Scalars`](../../reference/types/archetypes/scalars.md), for example with [`send_columns`](../send-data/send-columns.md).
