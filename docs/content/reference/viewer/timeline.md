---
title: Timeline
order: 3
---

Timeline controls
--------------------------

The timeline controls sit at the top of the timeline panel and allow you to control the playback and what [timeline](../../concepts/timelines.md) is active.

![timeline controls](https://static.rerun.io/3bef52fea54917b5eaf4f8789cb7c106f27e5c53_timeline-controls.png)

It lets you select which timeline is currently active and control the replay of the timeline.
These controls let you stop play/pause/step/loop time just like a video player.
When looping you can choose to loop through the whole recording, or just a sub-section of timeline that you select.
The rate of playback can also be sped up or slowed down by adjusting the rate multiplier.

Streams
-------

The Streams panel can be hidden with the layout config buttons at the top right corner of the viewer.

![streams](https://static.rerun.io/751d6c07964614b9e3a205dc0b70efbcced439ff_streams.png)

On the right side you see circles for each logged [event](../../concepts/timelines.md) on the currently selected timeline over time.
You can use the mouse to scrub the vertical time selector line to jump to arbitrary moments in time.
The stream view allows panning with right click and zooming with `ctrl/cmd + scroll`.


The tree on the left shows you all Entities that were logged for this timeline.
When you expand an Entity you will see both the Components that are associated with it, as well as any child Entities.
Selecting Entities or Events in the Streams view shows additional information in the Selection panel about them respectively.

### Discontinuity skipping
Rerun automatically detects discontinuities in the selected timeline and will skip over them while playing.
This is particularly useful whenever you have large gaps in the timestamps of your data recordings.
Detected discontinuities are visualized with a zigzag cut in the timeline.

[TODO(#1150)](https://github.com/rerun-io/rerun/issues/1150): Allow adjusting or disabling the time-discontinuity collapsing.
