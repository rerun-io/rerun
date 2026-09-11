---
title: "Keep following the time cursor after zooming or panning"
hidden: true
type: feature
---

Time series and state timeline views now preserve cursor-relative time range boundaries when zooming or panning, even when the time cursor is outside the visible window.
This keeps cursor-relative ranges following playback instead of switching to fixed, absolute times.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/584224f1632756028b89e131773b0ce7b4a3f57f_preserve_relative_cursor.mov" type="video/quicktime" />
</video>
