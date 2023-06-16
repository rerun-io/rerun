---
title: Timeline
order: 3
---

Timeline controls
--------------------------

The timeline controls sit at the top of the timeline panel and allow you to control the playback and what [timeline](../../concepts/timelines.md) is active.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/a85cbec1719ea4b1d5f46e8404ded735a8c47821_timeline-controls_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d9ad41384ce1d4840ec5e63aaf29e49556847c6b_timeline-controls_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bb789b476b4d6ff58506371b25b90492b5deef2e_timeline-controls_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/fee224eba3615e91dd3ad66bb793ab37eeb51f90_timeline-controls_1200w.png">
  <img src="https://static.rerun.io/a6fe67dc68457f0223f2d76d368b8138902101ac_timeline-controls_full.png" alt="timeline controls">
</picture>


It lets you select which timeline is currently active and control the replay of the timeline.
These controls let you stop play/pause/step/loop time just like a video player.
When looping you can choose to loop through the whole recording, or just a sub-section of timeline that you select.
The rate of playback can also be sped up or slowed down by adjusting the rate multiplier.

Streams
-------

The Streams panel can be hidden with the layout config buttons at the top right corner of the viewer.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/2d48e68e858ac07444b24a887303f075ac8a14c9_streams_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/059cc0d921a02ae710e76fab62b58631ed092724_streams_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/02a9e7b576cd88242cc512cb9d87a06e91e1b670_streams_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/ce03f6bec1bf99b3440d133d89775d0e957dd74e_streams_1200w.png">
  <img src="https://static.rerun.io/6e5e2b449f6f6ea150d68cfceae7eccb96b1c379_streams_full.png" alt="stream view">
</picture>


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
