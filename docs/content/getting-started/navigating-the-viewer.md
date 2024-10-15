---
title: Navigating the viewer
order: 500
---

This guide will familiarize you with the basics of using the Rerun Viewer with an example dataset. By the end you should be comfortable with the following topics:

-   [Launching the demo](#launching-the-demo)
-   [The Viewer panels](#the-viewer-panels)
-   [Exploring data](#exploring-data)
-   [Navigating the timeline](#navigating-the-timeline)

Here is a preview of the dataset that we will be working with:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough0_preview/d63e6774d94ff403d51355bacdfee9a3e7751dcf/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough0_preview/d63e6774d94ff403d51355bacdfee9a3e7751dcf/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough0_preview/d63e6774d94ff403d51355bacdfee9a3e7751dcf/1024w.png">
  <img src="https://static.rerun.io/viewer_walkthrough0_preview/d63e6774d94ff403d51355bacdfee9a3e7751dcf/full.png" alt="viewer walkthrough dataset preview screenshot">
</picture>

The demo uses the output of the [COLMAP](https://colmap.github.io/) structure-from-motion pipeline on a small dataset.
Familiarity with structure-from-motion algorithms is not a prerequisite for following the guide. All you need to know is
that at a very high level, COLMAP processes a series of images, and by tracking identifiable "keypoints" from frame to
frame, it is able to reconstruct both a sparse representation of the scene as well as the positions of the camera used
to take the images.

## Prerequisites

Although the Rerun SDK is available in both Python and Rust, this walkthrough makes use the Python installation. Even if
you plan to use Rerun with Rust, we still recommend having a Rerun Python environment available for quick
experimentation and working with examples. You can either follow the [Python Quickstart](./quick-start/python.md) or simply run:

```bash
pip install rerun-sdk
```

You can also find `rerun-sdk` on [`conda`](https://github.com/conda-forge/rerun-sdk-feedstock).

## Launching an example

If you have already followed the Python Quickstart you may have already check the "Helix" integrated example. This time, we will use the "Structure from Motion" example.

Start by running the viewer:

```bash
$ rerun
```

_Note: If this is your first time launching Rerun you will see a notification about the Rerun anonymous data usage
policy. Rerun collects anonymous usage data to help improve the SDK, though you may choose to opt out if you would
like._

This will bring you the Rerun viewer's Welcome screen:

<img src="https://static.rerun.io/welcome-screen/91f9bb2beca6c88ec3bfcdbeb0377d9164457f48/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/welcome-screen/91f9bb2beca6c88ec3bfcdbeb0377d9164457f48/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/welcome-screen/91f9bb2beca6c88ec3bfcdbeb0377d9164457f48/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/welcome-screen/91f9bb2beca6c88ec3bfcdbeb0377d9164457f48/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/welcome-screen/91f9bb2beca6c88ec3bfcdbeb0377d9164457f48/1200w.png">
</picture>

From there you can chose the "Structure from Motion" example. A window that looks like this will appear:

<picture>
  <img src="https://static.rerun.io/viewer_walkthrough_car_open/b5fa19d6bee481142b01b253ff63eef4066e1c96/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough_car_open/b5fa19d6bee481142b01b253ff63eef4066e1c96/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough_car_open/b5fa19d6bee481142b01b253ff63eef4066e1c96/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough_car_open/b5fa19d6bee481142b01b253ff63eef4066e1c96/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough_car_open/b5fa19d6bee481142b01b253ff63eef4066e1c96/1200w.png">
</picture>

Depending on your display size, the panels may have a different arrangements. Further in this guide you will learn how you can change that.

## The Viewer panels

There are 5 main parts to this window:

-   In the middle of the screen is the [Viewport](../reference/viewer/viewport.md). This is where you see the rendered
    space views for your session.
-   On the left top is the [Recordings](../concepts/apps-and-recordings.md) panel. This is where you see the list of loaded
    recordings, corresponding to their applications. You can also navigate back to the welcome screen from there.
-   Under recordings there is the [Blueprint](../reference/viewer/blueprint.md) panel. This is where the different space views can be
    controlled.
-   On the right is the [Selection](../reference/viewer/selection.md) panel. This is where you see extra information
    and configuration information for things that you have selected.
-   On the bottom is the [Timeline](../reference/viewer/timeline.md) panel. This is where you can control the current
    point in time that is being viewed.

Each of the 3 sides has a corresponding button in the upper-right corner. Try clicking each of these to hide and
show the corresponding panel.

<picture>
  <img src="https://static.rerun.io/viewer_walkthrough_car_toggle_panels/438e5e3fd70da11d15426e1d33510c60e0128dc8/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough_car_toggle_panels/438e5e3fd70da11d15426e1d33510c60e0128dc8/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough_car_toggle_panels/438e5e3fd70da11d15426e1d33510c60e0128dc8/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough_car_toggle_panels/438e5e3fd70da11d15426e1d33510c60e0128dc8/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough_car_toggle_panels/438e5e3fd70da11d15426e1d33510c60e0128dc8/1200w.png">
</picture>

There are several ways to rearrange the viewer layout to your liking: through the Viewer [user interface](configure-the-viewer/interactively.md),
via [Blueprint API](configure-the-viewer/through-code-tutorial.md), or by [loading an .rbl file](configure-the-viewer/save-and-load.md).

## Exploring data

In Rerun, data is modeled using [entities](../concepts/entity-component.md) (essentially objects) that contain batches of [components](../reference/types/components.md)
that change over time. Each entity is identified by an entity path, which uses a hierarchical syntax to represent relationships between entities.
Let's explore an example of this hierarchy in our scene:

-   `/camera/image/keypoints` is an entity stream that contains 3 component streams. One of these components indicates that together, they form a [Points2D archetype](../reference/types/archetypes/points2d.md),
    representing point clouds that were detected and tracked in images.
-   The images themselves are represented by the parent entity `/camera/image`. This entity consist of 6 components: 4 form an [Image archetype](../reference/types/archetypes/image.md),
    while the remaining 2 correspond to a [pinhole projection](../reference/types/archetype/pinhole.md). The images are captures by the camera, and a pinhole projection defines the relationship between 2D and 3D space.
-   Both the images and pinhole projection are hierarchically dependent on the camera's position, which is described by the `/camera` entity. This entity includes a series of transforms that together form a [Transform3D archetype](../reference/types/archetypes/transform3d.md).

The complete hierarchy of logged entity streams and their related component streams could be found under `Streams` in the Timeline panel. You might also notice a hierarchical list of similar entities in the Blueprint panel. The key difference between these two panels is that Blueprint panel focuses on how the stream data is arranged in the Viewport. In other words, an entity might be logged once but displayed in multiple views.

### Hover and selection

You can easily identify the connections between the same entities across different panels through the visual highlights. Hovering over an entity will
display a popup with additional information about its content. Clicking on it will reveal more details in the [Selection panel](../reference/viewer/selection.md).

Try each of the following:

-   Hover over the image to see a zoomed-in preview
-   Click on the point cloud to select the whole cloud
-   With the point cloud selected, hover and click individual points

### Rotate, zoom, and pan

Clicking and dragging the contents of any view will move it. You can rotate 3D views, or pan 2D views and plots. You can
also zoom using ctrl+scrollwheel or pinch gestures on a trackpad. Most views can be restored to their default state by
double-clicking somewhere in the view. Every view has a "?" icon in the upper right hand corner. You can always mouse
over this icon to find out more information about the specific view.

<picture>
  <img src="https://static.rerun.io/viewer_walkthrough_car_question/a215bd2234a0484ca5187e34d119bacc1ff260cb/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough_car_question/a215bd2234a0484ca5187e34d119bacc1ff260cb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough_car_question/a215bd2234a0484ca5187e34d119bacc1ff260cb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough_car_question/a215bd2234a0484ca5187e34d119bacc1ff260cb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough_car_question/a215bd2234a0484ca5187e34d119bacc1ff260cb/1200w.png">
</picture>

Try each of the following:

-   Drag the camera image and zoom in on one of the stickers
-   Rotate the 3D point cloud
-   Right-click and drag a rectangle to see a zoomed-in region of the plot
-   Double-click in each of the views to return them to default

## Navigating the timeline

If you look at the Timeline panel at the bottom of the window, you will see a series of white dots. Each of those dots
represents a piece of data that was logged at a different point in time. In fact, if you hover over the dot, the context popup will give you more information about
the specific thing that was logged.

There are several ways to navigate through the timeline:

-   Move the time indicator by dragging it to a different point on the timeline.
    For certain timelines, you can also click on the frame number and manually type the desired frame.
-   Adjust the playback speed, and for some timelines, you can also modify the number of frames per second.
-   Use the play, pause, step, and loop controls to playback Rerun data, similar to how you would with a video file.

Try out the following:

-   Use the arrow buttons (or Arrow keys on your keyboard) to step forward and backwards by a single frame
-   Click play to watch the data update on its own
-   Hit space bar to stop and start the playback
-   Hold shift and drag in the timeline to select a region
-   Toggle the loop button to playback on a loop of either the whole recording or just the selection

### Selecting different timelines

The current view of timeline is showing the data organized by the _frame number_ at which it was logged. Using frame
numbers can be a helpful way to synchronize things that may not have been logged at precisely the same time. However,
it's possible to also view the data in the specific order that it was logged. Click on the drop-down that says "frame"
and switch it to "log_time." If you zoom in on the timeline (using ctrl+scrollwheel), you can see that these events were
all logged at slightly different times.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough7_log_time/b6a4ce41f51e338270240e394140bd4d8a68f6bf/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough7_log_time/b6a4ce41f51e338270240e394140bd4d8a68f6bf/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough7_log_time/b6a4ce41f51e338270240e394140bd4d8a68f6bf/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough7_log_time/b6a4ce41f51e338270240e394140bd4d8a68f6bf/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough7_log_time/b6a4ce41f51e338270240e394140bd4d8a68f6bf/full.png" alt="viewer walkthrough change timeline screenshot">
</picture>

Feel free to spend a bit of time looking at the data across the different timelines. When you are done, switch back
to the "frame" timeline and double-click the timeline panel to reset it to the default range.

One thing to notice is there is a gap in the timeline in the "frame" view. This dataset is actually missing a few
frames, and the timeline view of frames makes this easy to spot. This highlights the importance of applying meaningful
timestamps to your data as you log it. You also aren't limited to frame and log_time. Rerun lets you define your own
timelines however you would like. You can read more about timelines [here](../concepts/timelines.md).

## Conclusion

That brings us to the end of this walkthrough. To recap, you have learned how to:

-   Install the `rerun-sdk` pypi package.
-   Run the Rerun Viewer using the `rerun` command.
-   Open the examples integrated in the viewer.
-   Work with the [Blueprint](../reference/viewer/blueprint.md), [Selection](../reference/viewer/selection.md) and [Timeline](../reference/viewer/timeline.md) panels.
-   Rearrange space view layouts.
-   Explore data through hover and selection.
-   Change the time selection.
-   Switch between different timelines.

Again, if you ran into any issues following this guide, please don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose).

### Up next

-   [Get started](./quick-start) by writing a program to log data with the Rerun SDK.
-   Learn how to further [configure the viewer](./configure-the-viewer) to suit your data.
-   Explore other [examples of using Rerun](/examples).
-   Consult the [concept overview](../concepts.md) for more context on the ideas covered here.
