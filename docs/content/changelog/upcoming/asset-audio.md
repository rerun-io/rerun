---
title: Audio support
hidden: true
type: feature
---

### Audio: `AssetAudio` archetype and `AudioView`

Audio files (`.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`) can now be logged as-is with the new `AssetAudio` archetype, or imported by opening or dropping them into the viewer.
The new audio view shows the waveform and plays the audio while time is playing on a temporal timeline, starting from the time the asset was logged.
Playback follows the playback speed between 0.25x and 4x, and each view has its own volume setting.

AAC is limited to AAC-LC for now.

[`AssetAudio` reference](../reference/types/archetypes/asset_audio.md)
[`AudioView` reference](../reference/types/views/audio_view.md)
