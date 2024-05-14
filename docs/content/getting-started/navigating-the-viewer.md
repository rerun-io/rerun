---
title: Navigating the viewer
order: 500
---

This guide will familiarize you with the basics of using the Rerun Viewer with an example dataset. By the end you should
be comfortable with the following topics:

* [Launching the demo](#launching-the-demo)
* [The Viewer panels](#the-viewer-panels)
* [Exploring data](#exploring-data)
* [Navigating the timeline](#navigating-the-timeline)

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

<picture>
  <img src="https://static.rerun.io/welcome_screen/f3119e719c64d7c18e56ccd34e3ec0eff7039ef6/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/welcome_screen/f3119e719c64d7c18e56ccd34e3ec0eff7039ef6/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/welcome_screen/f3119e719c64d7c18e56ccd34e3ec0eff7039ef6/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/welcome_screen/f3119e719c64d7c18e56ccd34e3ec0eff7039ef6/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/welcome_screen/f3119e719c64d7c18e56ccd34e3ec0eff7039ef6/1200w.png">
</picture>

Click on the "View Examples" button, and then chose the "Structure from Motion" example. A window that looks like this will appear:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough1_first_launch/793d828d867a8d341cd3ec35bc553f2d65fba549/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough1_first_launch/793d828d867a8d341cd3ec35bc553f2d65fba549/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough1_first_launch/793d828d867a8d341cd3ec35bc553f2d65fba549/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough1_first_launch/793d828d867a8d341cd3ec35bc553f2d65fba549/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough1_first_launch/793d828d867a8d341cd3ec35bc553f2d65fba549/full.png" alt="viewer walkthrough first launch screenshot">
</picture>

Depending on your display size, the panels may have a different arrangements. This does not yet look like the initial
preview, but the remainder of this guide will walk you through how to configure the Viewer to meet your needs.

## The Viewer panels

There are 4 main parts to this window:

-   In the middle of the screen is the [Viewport](../reference/viewer/viewport.md). This is where you see the rendered
    space views for your session.
-   On the left is the [Blueprint](../reference/viewer/blueprint.md) panel. This is where the different space views can be
    controlled.
-   On the right is the [Selection](../reference/viewer/selection.md) panel. This is where you see extra information
    and configuration information for things that you have selected.
-   On the bottom is the [Timeline](../reference/viewer/timeline.md) panel. This is where you can control the current
    point in time that is being viewed.

Each of the 3 side panels has a corresponding button in the upper right corner. Try clicking each of these to hide and
show the corresponding panel.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough2_toggle_panel/26cba988d81f960832801bcda2c7d233c2b34401/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough2_toggle_panel/26cba988d81f960832801bcda2c7d233c2b34401/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough2_toggle_panel/26cba988d81f960832801bcda2c7d233c2b34401/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough2_toggle_panel/26cba988d81f960832801bcda2c7d233c2b34401/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough2_toggle_panel/26cba988d81f960832801bcda2c7d233c2b34401/full.png" alt="viewer walkthrough toggle panel screenshots">
</picture>

For now, leave the panels visible since we will use them through the remainder of this guide.

It is also possible to re-arrange the individual space views. Try grabbing any of the named tabs, such as `image` and
dragging it to different locations in the Viewport. You can also resize individual views by grabbing the edge of the
view.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough3_rearrangeOD/ed7299b15ae5795d023d196a821e667a1a50591a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough3_rearrangeOD/ed7299b15ae5795d023d196a821e667a1a50591a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough3_rearrangeOD/ed7299b15ae5795d023d196a821e667a1a50591a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough3_rearrangeOD/ed7299b15ae5795d023d196a821e667a1a50591a/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough3_rearrangeOD/ed7299b15ae5795d023d196a821e667a1a50591a/full.png" alt="viewer walkthrough rearrange panels screenshot">
</picture>

Feel free to move the views around until you are happy with the layout.

## Exploring data

The space views are where you can see the data that was actually logged. This scene has streams of data for 6 different
primitives, also known as [entities](../concepts/entity-component.md):

-   [images](../reference/types/archetypes/image.md) that were captured from a camera.
-   [2D keypoints](../reference/types/archetypes/points2d.md) that were detected and tracked in those images.
-   a [pinhole](../reference/types/archetypes/pinhole.md) camera model that describes the relationship between 2D and 3D space.
-   [3D points](../reference/types/archetypes/points3d.md) that were computed by the COLMAP slam pipeline.
-   A sequence of [transforms](../reference/types/archetypes/transform3d.md) describing the 3D location of the camera in space.
-   A [scalar](../reference/types/archetypes/scalar.md) error metric that was computed by the algorithm for each frame.

### Hover and selection

You can find out more about these entities by hovering over them in the different views. Hovering will bring up a
context popup with additional information. You can also click on entities to select them and see more details in the
[Selection panel](../reference/viewer/selection.md).

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough4_hover/a22d892b0f00474aac948a3fce751a8cf559072d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough4_hover/a22d892b0f00474aac948a3fce751a8cf559072d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough4_hover/a22d892b0f00474aac948a3fce751a8cf559072d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough4_hover/a22d892b0f00474aac948a3fce751a8cf559072d/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough4_hover/a22d892b0f00474aac948a3fce751a8cf559072d/full.png" alt="viewer walkthrough hover screenshot">
</picture>

Try each of the following:

-   Hover over the image to see a zoomed-in preview
-   Click on the point cloud to select the whole cloud
-   With the point cloud selected, hover and click individual points

Note that the views are actually connected. As you hover over points in the `/ (Spatial)` view you will see information
about the depth of the projection in the image view. Conversely as you hover over pixels in the `image` you will see the
corresponding ray projected into the `/ (Spatial)` view. See the section on
[Spaces and Transforms](../concepts/spaces-and-transforms.md) for more information on how this linking works.

### Rotate, zoom, and pan

Clicking and dragging the contents of any view will move it. You can rotate 3D views, or pan 2D views and plots. You can
also zoom using ctrl+scrollwheel or pinch gestures on a trackpad. Most views can be restored to their default state by
double-clicking somewhere in the view. Every view has a "?" icon in the upper right hand corner. You can always mouse
over this icon to find out more information about the specific view.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough5_nav/7847244e2657a5555d90f4dd804e2650e4fde527/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough5_nav/7847244e2657a5555d90f4dd804e2650e4fde527/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough5_nav/7847244e2657a5555d90f4dd804e2650e4fde527/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough5_nav/7847244e2657a5555d90f4dd804e2650e4fde527/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough5_nav/7847244e2657a5555d90f4dd804e2650e4fde527/full.png" alt="viewer walkthrough rotate zoom and pan screenshot">
</picture>

Try each of the following:

-   Drag the camera image and zoom in on one of the stickers
-   Rotate the 3D point cloud
-   Right-click and drag a rectangle to see a zoomed-in region of the plot
-   Double-click in each of the views to return them to default

## Navigating the timeline

So far, we have only been exploring data from a single point in time. However, if you look at the Timeline panel at the
bottom of the window, you will see a series of white dots. Each of those dots represents a piece of data that was logged
at a different point in time. In fact, if you hover over the dot, the context popup will give you more information about
the specific thing that was logged.

### Changing the time slider

To change the position on the timeline, simply grab the time indicator and pull it to the point in time you are
interested in seeing. The space views will adjust accordingly. You can also use the play/pause/step/loop controls to
playback the Rerun data as you might with a video file.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough6_timeline/9816d7becf19399735bef1f17f1d4bb928c278f7/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough6_timeline/9816d7becf19399735bef1f17f1d4bb928c278f7/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough6_timeline/9816d7becf19399735bef1f17f1d4bb928c278f7/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough6_timeline/9816d7becf19399735bef1f17f1d4bb928c278f7/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough6_timeline/9816d7becf19399735bef1f17f1d4bb928c278f7/full.png" alt="viewer walkthrough timeline screenshot">
</picture>

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

- [Get started](./quick-start) by writing a program to log data with the Rerun SDK.
- Learn how to further [configure the viewer](./configure-the-viewer) to suit your data.
- Explore other [examples of using Rerun](/examples).
- Consult the [concept overview](../concepts.md) for more context on the ideas covered here.
