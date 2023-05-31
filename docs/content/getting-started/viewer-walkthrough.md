---
title: Viewer Walkthrough
order: 3
---

This guide will familiarize you with the basics of using the Rerun Viewer with an example dataset. By the end you should
be comfortable with the following topics:
 * [Launching the demo](#launching-the-demo)
 * [The viewer panels](#the-viewer-panels)
 * [Exploring data](#exploring-data)
 * [Navigating the timeline](#navigating-the-timeline)
 * [Configuring views](#configuring-views)
 * [Creating new views](#creating-new-views)

Here is a preview of the dataset that we will be working with:

![Preview](/docs-media/viewer_walkthrough0_preview.png)

The demo uses the output of the [COLMAP](https://colmap.github.io/) structure-from-motion pipeline on a small dataset.
Familiarity with structure-from-motion algorithms is not a prerequisite for following the guide. All you need to know is
that at a very high level, COLMAP processes a series of images, and by tracking identifiable "keypoints" from frame to
frame, it is able to reconstruct both a sparse representation of the scene as well as the positions of the camera used
to take the images.

## Prerequisites
Although the Rerun SDK is available in both Python and Rust, this walkthrough makes use the Python installation. Even if
you plan to use Rerun with Rust, we still recommend having a Rerun Python environment available for quick
experimentation and working with examples. You can either follow the [Python Quickstart](python.md) or simply run:

```bash
pip install rerun-sdk
```

## Launching the demo

If you have already followed the Python Quickstart you may have used `rerun_demo` already to run the cube demo.

This time, we will pass an additional flag:
```bash
$ python -m rerun_demo --colmap
```

*Note: If this is your first time launching Rerun you will see a notification about the Rerun anonymous data usage
policy. Rerun collects anonymous usage data to help improve the SDK, though you may choose to opt out if you would
like.*

In your terminal you should see an output along the lines of:
```
2023-02-13T05:16:06.835424Z  INFO rerun::run: Loading "/home/rerun/venv/lib/python3.10/site-packages/rerun_sdk/rerun_demo/colmap.rrd"…
```

And a window that looks like this will appear:

![First Launch](/docs-media/viewer_walkthrough1_first_launch.png)

Depending on your display size, the panels may have a different arrangements. This does not yet look like the initial
preview, but the remainder of this guide will walk you through how to configure the Viewer to meet your needs.

## The viewer panels

There are 4 main parts to this window:
- In the middle of the screen is the [Viewport](../reference/viewer/viewport.md). This is where you see the rendered
  space views for your session.
- On the left is the [Blueprint](../reference/viewer/blueprint.md) panel. This is where the different space views can be
  controlled.
- On the right is the [Selection](../reference/viewer/selection.md) panel. This is where you see extra information
  and configuration information for things that you have selected.
- On the bottom is the [Timeline](../reference/viewer/timeline.md) panel. This is where you can control the current
  point in time that is being viewed.

Each of the 3 side panels has a corresponding button in the upper right corner. Try clicking each of these to hide and
show the corresponding panel.

![Toggle Panel](/docs-media/viewer_walkthrough2_toggle_panel.png)

For now, leave the panels visible since we will use them through the remainder of this guide.

It is also possible to re-arrange the individual space views. Try grabbing any of the named tabs, such as `image` and
dragging it to different locations in the Viewport. You can also resize individual views by grabbing the edge of the
view.

![Rearrange Views](/docs-media/viewer_walkthrough3_rearrange.png)

Feel free to move the views around until you are happy with the layout.

## Exploring data
The space views are where you can see the data that was actually logged. This scene has streams of data for 6 different
primitives, also known as [entities](../concepts/entity-component.md):
* [images](../reference/primitives.md#tensors--images) that were captured from a camera.
* [2d keypoints](../reference/primitives.md#point-2d) that were detected and tracked in those images.
* a [camera](../reference/primitives.md#pinhole) model that describes the relationship between 2D and 3D space.
* [3d points](../reference/primitives.md#point-3d) that were computed by the COLMAP slam pipeline.
* A sequence of [transforms](../reference/primitives.md#rigid3d) describing the 3D location of the camera in space.
* A [scalar](../reference/primitives.md#scalar) error metric that was computed by the algorithm for each frame.

### Hover and selection
You can find out more about these entities by hovering over them in the different views. Hovering will bring up a
context popup with additional information. You can also click on entities to select them and see more details in the
[Selection panel](../reference/viewer/selection.md).

![Hover Data](/docs-media/viewer_walkthrough4_hover.png)

Try each of the following:
 * Hover over the image to see a zoomed-in preview
 * Click on the point cloud to select the whole cloud
 * With the point cloud selected, hover and click individual points

Note that the views are actually connected. As you hover over points in the `/ (Spatial)` view you will see information
about the depth of the projection in the image view. Conversely as you hover over pixels in the `image` you will see the
corresponding ray projected into the `/ (Spatial)` view. See the section on
[Spaces and Transforms](../concepts/spaces-and-transforms.md) for more information on how this linking works.

### Rotate, zoom, and pan
Clicking and dragging the contents of any view will move it. You can rotate 3d views, or pan 2d views and plots. You can
also zoom using ctrl+scrollwheel or pinch gestures on a trackpad. Most views can be restored to their default state by
double-clicking somewhere in the view. Every view has a "?" icon in the upper right hand corner. You can always mouse
over this icon to find out more information about the specific view.

![Adjust Scene Views](/docs-media/viewer_walkthrough5_nav.png)

Try each of the following:
 * Drag the camera image and zoom in on one of the stickers
 * Rotate the 3D point cloud
 * Right-click and drag a rectangle to see a zoomed-in region of the plot
 * Double-click in each of the views to return them to default

## Navigating the timeline
So far, we have only been exploring data from a single point in time. However, if you look at the Timeline panel at the
bottom of the window, you will see a series of white dots. Each of those dots represents a piece of data that was logged
at a different point in time. In fact, if you hover over the dot, the context popup will give you more information about
the specific thing that was logged.

### Changing the time slider
To change the position on the timeline, simply grab the time indicator and pull it to the point in time you are
interested in seeing.  The space views will adjust accordingly. You can also use the play/pause/step/loop controls to
playback the Rerun data as you might with a video file.

![Adjust Time Slider](/docs-media/viewer_walkthrough6_timeline.png)

Try out the following:
  * Use the arrow buttons (or arrow keys on your keyboard) to step forward and backwards by a single frame
  * Click play to watch the data update on its own
  * Hit space bar to stop and start the playback
  * Hold shift and drag in the timeline to select a region
  * Toggle the loop button to playback on a loop of either the whole recording or just the selection

### Selecting different timelines
The current view of timeline is showing the data organized by the *frame number* at which it was logged. Using frame
numbers can be a helpful way to synchronize things that may not have been logged at precisely the same time. However,
it's possible to also view the data in the specific order that it was logged.  Click on the drop-down that says "frame"
and switch it to "log_time." If you zoom in on the timeline (using ctrl+scrollwheel), you can see that these events were
all logged at slightly different times.

![Log Time](/docs-media/viewer_walkthrough7_log_time.png)

Feel free to spend a bit of time looking at the data across the different timelines. When you are done, switch back
to the "frame" timeline and double-click the timeline panel to reset it to the default range.

One thing to notice is there is a gap in the timeline in the "frame" view. This dataset is actually missing a few
frames, and the timeline view of frames makes this easy to spot. This highlights the importance of applying meaningful
timestamps to your data as you log it. You also aren't limited to frame and log_time. Rerun lets you define your own
timelines however you would like. You can read more about timelines [here](../concepts/timelines.md).

## Configuring views
Views in Rerun are configured by [Blueprints](../reference/viewer/blueprint.md). We will now use blueprints to adjust
both an individual entity as well as the contents of a space view itself.

### Adjusting entity properties
First, click to select the entity named `points` in the  `/ (Spatial)` view in the Blueprint panel. Now, look and the
selection panel -- in addition to the information about the data associated with that entity, you will see a "Blueprint"
section.

Try toggling "visible" on and off and you will see that the points disappear and reappear. Next, click the control
labeled "visible history" and drag it to the right to increase the value. As you drag farther you will see more points
show up in the view. This is making historical points, from farther back in time visible within the time point of this
view. Because the points are logged in stationary 3d space, aggregating them here gives us a more complete view of the
car. Leave the visible history with a value of 50.

![Visible History](/docs-media/viewer_walkthrough8_history.png)

### Modifying the contents of a space view
Now select the `/ (Spatial)` view itself. We will start by giving this space view a different name. At the very
top of the selection panel you will see a text box labeled "Space view:". Go ahead and change the name to
`Reconstruction`. The name will also update in the blueprint panel on the left.

Like with the entity selection, you will see a Blueprint section within the Selection panel. This time, click on the
button labeled "Add/Remove Entities". This pop-up shows all of the entities that were logged as part of this session.
You can click on the "+" or "-" buttons to add or remove entities from this view. Go ahead and remove the entity called
"keypoints," and then add them back again. Unlike hiding an entity, you will notice that as you remove entities they
completely disappear from the blueprint panel on the left. Entities that are incompatible with the selected view will be
grayed out. For example, you cannot add a scalar to a spatial scene.

![Add/Remove Entities](/docs-media/viewer_walkthrough9_add_remove.png)

## Creating new views
New views can be created using the "+" button at the top of the Blueprint panel. When you click this button you will
need to choose a root for your new space. This is the space that will act as your origin within the
[transform system](../concepts/spaces-and-transforms.md).

![Create a view](/docs-media/viewer_walkthrough10_create.png)

After creating this new view, your view layout might be feeling a little cluttered. You can quickly hide views you're
not using from the blueprint panel by hovering over the view and then clicking the icon that looks like an eye. Go ahead
and hide the `image` and `avg_reproj_err` views, and collapse the expanded timeline panel using the button in the upper
right corner. Note that even with the timeline collapsed you still have access to timeline controls, including a slider.

![Toggle Vis](/docs-media/viewer_walkthrough11_toggle_vis.png)

### Reusing what you've learned
Finally, use what we covered in the previous section to change the contents of this view. Select the new `camera` view,
then choose "Add/remove entities." Remove the 2d "keypoints" and add in the 3d "points." Note that these points do not
have visible history turned on -- that's because the blueprint is part of the view and not part of the entity.
Select the points within this view by clicking on them in the blueprint or the view itself, and then give them visible
history as well. When you are done, your view should look like this:

![Camera View](/docs-media/viewer_walkthrough12_cameraview.png)

Now move the slider back and forth and see what happens. Even though they are both views of the same camera and point
entities, they behave quite differently. On the top the camera moves relative to the car, while on the bottom the car
moves relative to the camera. This is because the new views have *different* space roots, and Rerun uses the transform
system to transform or project all data into the space root for the given view.

## Conclusion

That brings us to the end of this walkthrough. To recap, you have learned how to:
- Install the `rerun-sdk` pypi package.
- Run the Rerun Viewer using the `rerun_demo` helper.
- Work with the [Blueprint](../reference/viewer/blueprint.md), [Selection](../reference/viewer/selection.md) and [Timeline](../reference/viewer/timeline.md) panels.
- Rearrange space view layouts.
- Explore data through hover and selection.
- Change the time selection.
- Switch between different timelines.
- Configure entity blueprint properties.
- Add and remove entities from views.
- Create and configure new views.
- And some basics of how transforms work.

Again, if you ran into any issues following this guide, please don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose).

### Up next
To get started with writing a program to logging data with the Rerun SDK see the [Python](logging-python.md) or
[Rust](logging-rust.md) getting started guides.

To see and explore other data, you can check out the [examples](examples.md).

For deeper context on the ideas covered here, consult the [Concept overview](../concepts.md).
