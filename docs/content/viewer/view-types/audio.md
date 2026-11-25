---
title: Audio
order: 130
description: Listen to audio and inspect its waveform
---

Use an audio view to inspect an audio asset's waveform and play it back.
It is useful for relating sounds to events in other views.
Audio plays with a temporal timeline, starting at the time the asset was logged.

See the [`AudioView` reference](../../reference/types/views/audio_view.md) for properties and examples.

## Data for this view

Log an [`AssetAudio`](../../reference/types/archetypes/asset_audio.md) at the time playback should start on a temporal timeline.
Supported files include `.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, and `.wav`.

## Configure the view

Use the view's playback settings to control volume and muting.
See the [AudioView reference](../../reference/types/views/audio_view.md).

## Limitations

Raw PCM streaming is not supported; encode samples as WAV first.
To plot the signal, log it as [`Scalars`](../../reference/types/archetypes/scalars.md).

## Related

See [Navigate a recording layout](../inspect-recordings/configure-layout.md) and [Configure a recording layout from code](../inspect-recordings/configure-from-code.md).
