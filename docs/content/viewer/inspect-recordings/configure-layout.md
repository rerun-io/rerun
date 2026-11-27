---
title: Navigate a recording layout
order: 100
---

## Panels

**1** — Sources panel; **2** — Blueprint panel; **3** — Viewport; **4** — Selection panel; **5** — Timeline panel

<picture>
  <img src="https://static.rerun.io/viewer-layout-recording/4f5429a2b818d1145a8712a83033beb57433632f/full.png" alt="Viewer showing five numbered areas: Sources panel, blueprint panel, viewport, selection panel, and timeline panel">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer-layout-recording/4f5429a2b818d1145a8712a83033beb57433632f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer-layout-recording/4f5429a2b818d1145a8712a83033beb57433632f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer-layout-recording/4f5429a2b818d1145a8712a83033beb57433632f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer-layout-recording/4f5429a2b818d1145a8712a83033beb57433632f/1200w.png">
</picture>

## Command palette

Open the command palette with `Cmd`/`Ctrl` + `K` or `Cmd`/`Ctrl` + `P`, the search box in the top bar, or the menu.
Use it to search for Viewer commands, entities and components in the active recording, or servers, datasets, and tables known to the Viewer.
You can also enter a URL or file path to open it.

<picture>
  <img class="d2-dark" src="https://static.rerun.io/b82ee349ee7c26eb389961105819f6a7dbef76ad_command-palette-dark.png" alt="Command palette showing a search field, an agent mode hint, and a list of Viewer commands and shortcuts">
  <img class="d2-light" src="https://static.rerun.io/4d2337767e976c83fc5f0bcb2e7d8cc68af68765_command-palette-light.png" alt="Command palette showing a search field, an agent mode hint, and a list of Viewer commands and shortcuts">
</picture>

Press `Tab` to switch to agent mode, where you can send a prompt to the [agent panel](../../agents.md).
If you haven't set up an agent yet, the palette offers a button to do so.

## Timeline panel

The timeline panel controls playback, picks the active timeline, and shows when each entity received data.

### Timeline controls

The timeline controls sit at the top of the timeline panel and allow you to control the playback and what [timeline](../../log/data-model/timelines.md) is active.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/1200w.png">
  <img src="https://static.rerun.io/timeline-controls/bacd4d3d0ff2dd812bf0502d5e03689d82711b64/full.png" alt="timeline controls">
</picture>


They play, pause, step, and loop time like a video player.
When looping you can choose to loop through the whole recording, or just a sub-section of timeline that you select.
The rate of playback can also be sped up or slowed down by adjusting the rate multiplier.

### Streams

The streams panel can be hidden with the layout config buttons at the top right corner of the viewer.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/1200w.png">
  <img src="https://static.rerun.io/streams/376becde1280bcbc993add31cf37df0539622651/full.png" alt="streams panel">
</picture>


On the right side you see circles for each logged [event](../../log/data-model/timelines.md) on the currently selected timeline over time.
You can use the mouse to scrub the vertical time selector line to jump to arbitrary moments in time.
The streams panel allows panning with right click and zooming with `ctrl/cmd + scroll`.


The tree on the left shows you all entities that were logged for this timeline.
When you expand an entity you will see both the components that are associated with it, as well as any child entities.
Selecting entities or events in the streams panel shows additional information in the selection panel about them respectively.

### Discontinuity skipping
Rerun automatically detects discontinuities in the selected timeline and will skip over them while playing.
This is particularly useful whenever you have large gaps in the timestamps of your data recordings.
Detected discontinuities are visualized with a zigzag cut in the timeline.

Discontinuity skipping cannot be adjusted or turned off yet ([#1150](https://github.com/rerun-io/rerun/issues/1150)).
<!-- TODO(#1150): update this when skipping becomes configurable. -->


## Viewport

The viewport is a flexible area where you can arrange your views:
You can grab the title of any view to dock it to different parts of the viewport or to form tabs.

## Blueprint panel

The blueprint panel shows the recording layout as a tree of containers, views, and the entities in each view.
Use the eye icon to show or hide an item, or the remove button to remove it from the layout.
Select an entity in a view to change its visualizers and other per-view settings in the selection panel.

See [Customize your visualization](../view-concepts/customize-views.md) for visualizers and overrides, and [Recording layout and blueprints](recording-layout.md) for saving and resetting layouts.

<span id="dataset-blueprints"></span>

For recordings from a catalog dataset, see [Select a recording blueprint](recording-layout.md#select-a-recording-blueprint-for-a-dataset).

## Adding entities to views

To add or re-add an entity to a view:
1. Select the target view in the blueprint panel
2. Click the button in the [Selection Panel](configure-layout.md#selection-panel) to open the entity picker
3. Select entities to add (only compatible entities for that view class are shown)

See [Entity path filter](../view-concepts/entity-path-filter.md) for information on how view content is determined.

## Selection panel

Select an item in a view, the blueprint panel, or the timeline panel to inspect its data and settings.
For an entity in a view, you can also change its visualizers and per-view settings here.
Use the back and forward arrows to revisit selections, or hold `cmd/ctrl` while clicking to select multiple items.

### Linking views
Hovering and selection are shared by all views.
When you hover or select an entity, or one instance of it (for example one point of a point cloud), it is highlighted in every view that shows the same entity path and instance.

This only links the _same_ entity path.
Different entities are not linked, even if they describe the same object, for example `/camera/image` and `/world/points`.
To link them, give the related data the same entity path, or build your own linking with [callbacks](../extend/callbacks.md).

Time is also shared: all views show the same time cursor.
Right-click in a time series view to move the time cursor to that time.
To pan and zoom the time axis of several plots together, see [linked time axes](../view-concepts/visible-time-ranges.md#link-the-time-axis-of-several-plots).
