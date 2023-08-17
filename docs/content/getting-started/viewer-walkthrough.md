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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/74a37346b0a1f47a8dc6d57d2dbc01ee6afb3960_viewer_walkthrough0_preview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/1d645a8fd2eb8f874bbbad3be084c96fe0c9bd39_viewer_walkthrough0_preview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/6708f38efc707557d4ce15b5e67dddd7782d5051_viewer_walkthrough0_preview_1024w.png">
  <img src="https://static.rerun.io/d63e6774d94ff403d51355bacdfee9a3e7751dcf_viewer_walkthrough0_preview_full.png" alt="viewer walkthrough dataset preview screenshot">
</picture>


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

You can also find `rerun-sdk` on [`conda`](https://github.com/conda-forge/rerun-sdk-feedstock).

## Launching the demo

If you have already followed the Python Quickstart you may have used `rerun_demo` already to run the cube demo.

This time, we will pass an additional flag:
```bash
$ python -m rerun_demo --structure-from-motion
```

*Note: If this is your first time launching Rerun you will see a notification about the Rerun anonymous data usage
policy. Rerun collects anonymous usage data to help improve the SDK, though you may choose to opt out if you would
like.*

In your terminal you should see an output along the lines of:
```
2023-02-13T05:16:06.835424Z  INFO rerun::run: Loading "/home/rerun/venv/lib/python3.10/site-packages/rerun_sdk/rerun_demo/colmap_fiat.rrd"…
```

And a window that looks like this will appear:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/e3ed0032e35937f22ab473a63f9a44dbe9fcc519_viewer_walkthrough1_first_launch_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/3b0bff307be3711c4bcf1fa1e54a1808ddccafe6_viewer_walkthrough1_first_launch_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bcf6a979f66df8d5435b958ee3286b80604b161e_viewer_walkthrough1_first_launch_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/4d7d0e9933abb124253ae9455933ba40fcb72fb4_viewer_walkthrough1_first_launch_1200w.png">
  <img src="https://static.rerun.io/793d828d867a8d341cd3ec35bc553f2d65fba549_viewer_walkthrough1_first_launch_full.png" alt="viewer walkthrough first launch screenshot">
</picture>


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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/65b3a60d28105aaabb1675e37f998f6fe4bd3f5a_viewer_walkthrough2_toggle_panel_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/6f71431a2946fa7ae0f5033b90ca23c664cc4856_viewer_walkthrough2_toggle_panel_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/b6a5ec42e21a88a55d1131501a4fd53b860290e9_viewer_walkthrough2_toggle_panel_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/5d3a114ba9b8695fa5897e6600a9ea58d57665fe_viewer_walkthrough2_toggle_panel_1200w.png">
  <img src="https://static.rerun.io/26cba988d81f960832801bcda2c7d233c2b34401_viewer_walkthrough2_toggle_panel_full.png" alt="viewer walkthrough toggle panel screenshots">
</picture>

For now, leave the panels visible since we will use them through the remainder of this guide.

It is also possible to re-arrange the individual space views. Try grabbing any of the named tabs, such as `image` and
dragging it to different locations in the Viewport. You can also resize individual views by grabbing the edge of the
view.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/27a97272519fc3f4185cb43d0ba80fbef5a5408e_viewer_walkthrough3_rearrangeOD_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/1c868e97537c1625c6bec0051bf7b7d184f79ebc_viewer_walkthrough3_rearrangeOD_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bc7ee5a9a1a4a95f6ef8b1f29fb4ad73605833c5_viewer_walkthrough3_rearrangeOD_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/62e5c42de6c545b9b85582a6bb91a4feb8dccf99_viewer_walkthrough3_rearrangeOD_1200w.png">
  <img src="https://static.rerun.io/ed7299b15ae5795d023d196a821e667a1a50591a_viewer_walkthrough3_rearrangeOD_full.png" alt="viewer walkthrough rearrange panels screenshot">
</picture>


Feel free to move the views around until you are happy with the layout.

## Exploring data
The space views are where you can see the data that was actually logged. This scene has streams of data for 6 different
primitives, also known as [entities](../concepts/entity-component.md):
* [images](../reference/data_types/image.md) that were captured from a camera.
* [2d keypoints](../reference/data_types/point2d.md) that were detected and tracked in those images.
* a [pinhole](../reference/data_types/pinhole.md) camera model that describes the relationship between 2D and 3D space.
* [3d points](../reference/data_types/point3d.md) that were computed by the COLMAP slam pipeline.
* A sequence of [transforms](../reference/data_types/transform3d.md) describing the 3D location of the camera in space.
* A [scalar](../reference/data_types/scalar.md) error metric that was computed by the algorithm for each frame.

### Hover and selection
You can find out more about these entities by hovering over them in the different views. Hovering will bring up a
context popup with additional information. You can also click on entities to select them and see more details in the
[Selection panel](../reference/viewer/selection.md).

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/f99a8f63f0b653dfac91476a60b3482edf8e638f_viewer_walkthrough4_hover_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/9567d3d33e8e6743de3f08c65fd3a285607bd082_viewer_walkthrough4_hover_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/f6a667b741dd05bed739a3e6838e33653af2e65c_viewer_walkthrough4_hover_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/90d296b819bf5f355853a01bfc7686265fd96905_viewer_walkthrough4_hover_1200w.png">
  <img src="https://static.rerun.io/a22d892b0f00474aac948a3fce751a8cf559072d_viewer_walkthrough4_hover_full.png" alt="viewer walkthrough hover screenshot">
</picture>


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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/b87329150e8096e585956da2eebabe26219ca14f_viewer_walkthrough5_nav_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/ecb03b4fda5b0c55f06ab7252aacd3dbc7d883f6_viewer_walkthrough5_nav_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/86344228e1470c13888980f8441d774eb8d32315_viewer_walkthrough5_nav_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/33e87380305028a30b87f1303b588a8ee02a6f74_viewer_walkthrough5_nav_1200w.png">
  <img src="https://static.rerun.io/7847244e2657a5555d90f4dd804e2650e4fde527_viewer_walkthrough5_nav_full.png" alt="viewer walkthrough rotate zoom and pan screenshot">
</picture>


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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/856f760b910a74a7ca0984b8ab06b3d2f67c5bcb_viewer_walkthrough6_timeline_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/2fa88a9a48ce575ec573fa71882137e8313e1454_viewer_walkthrough6_timeline_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/89772c671cf1bf3743b4d665122849968013ef95_viewer_walkthrough6_timeline_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/b196787f097b1ef4ba953a0487c1a1c48227c794_viewer_walkthrough6_timeline_1200w.png">
  <img src="https://static.rerun.io/9816d7becf19399735bef1f17f1d4bb928c278f7_viewer_walkthrough6_timeline_full.png" alt="viewer walkthrough timeline screenshot">
</picture>


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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/38a4a41b5814cbe157cb766440796047a369635f_viewer_walkthrough7_log_time_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/10741579d589ad806bdcdd39b05c51d1e11f501b_viewer_walkthrough7_log_time_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/210d91597ce9cea606ce8abfe3ea9c5ef7df7867_viewer_walkthrough7_log_time_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/e07ae99d7d1f36dbe19fd36facd2f354ac93ae01_viewer_walkthrough7_log_time_1200w.png">
  <img src="https://static.rerun.io/b6a4ce41f51e338270240e394140bd4d8a68f6bf_viewer_walkthrough7_log_time_full.png" alt="viewer walkthrough change timeline screenshot">
</picture>


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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/90fd21b61415776eafab8c9df07a5ea5df5186fd_viewer_walkthrough8_history_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/794677a662ba4dd755dadd118f1adf272da462eb_viewer_walkthrough8_history_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/25a6ac53a10461936ae019756ff3c4d53b7628c3_viewer_walkthrough8_history_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/be10b402dedc476ee1ac4d11402de6331b82fd83_viewer_walkthrough8_history_1200w.png">
  <img src="https://static.rerun.io/9c6a01f4dd2059641d92d121f8f2772203c56cfa_viewer_walkthrough8_history_full.png" alt="viewer walkthrough adjusting visible history screenshot">
</picture>

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

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/982cd744bcbcdffc5161b84cae0c5b32fc58e819_viewer_walkthrough9_add_remove_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/90616f9dce6f28b18d4c4ee1d5750d43666a9748_viewer_walkthrough9_add_remove_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/a8669d788115df5671da92d814fa96951c569916_viewer_walkthrough9_add_remove_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/73bf1e30fe6878866fa1bc5e7652b30902f53506_viewer_walkthrough9_add_remove_1200w.png">
  <img src="https://static.rerun.io/e22b231be49391998d6e6ef005b2dad0a85d2adf_viewer_walkthrough9_add_remove_full.png" alt="viewer walkthrough modifying contents of a space view screenshot">
</picture>


## Creating new views
New views can be created using the "+" button at the top of the Blueprint panel. When you click this button you will
need to choose a root for your new space. This is the space that will act as your origin within the
[transform system](../concepts/spaces-and-transforms.md).

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/1bcf92925fceb39810a90110b26a1b26e2f6f5dc_viewer_walkthrough10_create_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/f621f5caf31bd0b7648b38bf10d8b3713c6ae043_viewer_walkthrough10_create_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/4267a02133317e071dabd92551567a84dc0136eb_viewer_walkthrough10_create_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/3b4788a2acd3823649589a8a4a52d915d12daed0_viewer_walkthrough10_create_1200w.png">
  <img src="https://static.rerun.io/d89060f824af6b3f188e9187b8b5b9b1d7f75646_viewer_walkthrough10_create_full.png" alt="viewer walkthrough creating new view screenshot">
</picture>


After creating this new view, your view layout might be feeling a little cluttered. You can quickly hide views you're
not using from the blueprint panel by hovering over the view and then clicking the icon that looks like an eye. Go ahead
and hide the `image` and `avg_reproj_err` views, and collapse the expanded timeline panel using the button in the upper
right corner. Note that even with the timeline collapsed you still have access to timeline controls, including a slider.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/526e5dba02d3786045ff2b1d6dc20ade23325e4e_viewer_walkthrough11_toggle_vis_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/aa778e93172344fc1f340815564fa28ee33dcc54_viewer_walkthrough11_toggle_vis_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/dcc33a360cf506b0b8dd61fe4f24c9030a0d0467_viewer_walkthrough11_toggle_vis_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/105b90940ff92ad296ef2d5dd74af9ca1256f4a6_viewer_walkthrough11_toggle_vis_1200w.png">
  <img src="https://static.rerun.io/28d2720b63fbb2f3d3def0f37962f1ace3674085_viewer_walkthrough11_toggle_vis_full.png" alt="viewer walkthrough toggle visibility screenshot">
</picture>


### Reusing what you've learned
Finally, use what we covered in the previous section to change the contents of this view. Select the new `camera` view,
then choose "Add/remove entities." Remove the 2d "keypoints" and add in the 3d "points." Note that these points do not
have visible history turned on -- that's because the blueprint is part of the view and not part of the entity.
Select the points within this view by clicking on them in the blueprint or the view itself, and then give them visible
history as well. When you are done, your view should look like this:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/2bb43fcc8e3853d56ceb76f2db653d4c86331d5b_viewer_walkthrough12_cameraview_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/fd2a3990227a119cdc0adca137bd47f732ceaf7d_viewer_walkthrough12_cameraview_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/d6da109e6e4d6c71da6c6d18c18685713b3df917_viewer_walkthrough12_cameraview_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/0548051977569faf44ec3487b8c6285d4ca761f3_viewer_walkthrough12_cameraview_1200w.png">
  <img src="https://static.rerun.io/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb_viewer_walkthrough12_cameraview_full.png" alt="viewer walkthrough camera view screenshot">
</picture>


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

To see and explore other data, you can check out the [examples](/examples).

For deeper context on the ideas covered here, consult the [Concept overview](../concepts.md).
