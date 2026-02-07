---
title: Timeline
order: 3
---

Timeline controls
--------------------------

The timeline controls sit at the top of the timeline panel and allow you to control the playback and what [timeline](../../concepts/logging-and-ingestion/timelines.md) is active.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/1200w.png">
  <img src="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/full.png" alt="timeline controls">
</picture>


It lets you select which timeline is currently active and control the replay of the timeline.
These controls let you stop play/pause/step/loop time just like a video player.
When looping you can choose to loop through the whole recording, or just a sub-section of timeline that you select.
The rate of playback can also be sped up or slowed down by adjusting the rate multiplier.

Streams
-------

The Streams panel can be hidden with the layout config buttons at the top right corner of the viewer.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/1200w.png">
  <img src="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/full.png" alt="stream view">
</picture>


On the right side you see circles for each logged [event](../../concepts/logging-and-ingestion/timelines.md) on the currently selected timeline over time.
You can use the mouse to scrub the vertical time selector line to jump to arbitrary moments in time.
The stream view allows panning with right click and zooming with `ctrl/cmd + scroll`.


The tree on the left shows you all entities that were logged for this timeline.
When you expand an entity you will see both the components that are associated with it, as well as any child entities.
Selecting entities or events in the streams view shows additional information in the selection panel about them respectively.

### Discontinuity skipping
Rerun automatically detects discontinuities in the selected timeline and will skip over them while playing.
This is particularly useful whenever you have large gaps in the timestamps of your data recordings.
Detected discontinuities are visualized with a zigzag cut in the timeline.

[TODO(#1150)](https://github.com/rerun-io/rerun/issues/1150): Allow adjusting or disabling the time-discontinuity collapsing.
