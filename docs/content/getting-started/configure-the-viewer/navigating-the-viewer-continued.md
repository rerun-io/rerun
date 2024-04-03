---
title: Navigating the viewer (continued)
order: 4
---

This guide builds on top of the previous tutorial:
[Navigating the viewer](../navigating-the-viewer.md). Please follow that tutorial first if you haven't already.

This guide will familiarize you with the basics of using the Rerun Viewer with an example dataset. By the end you should
be comfortable with the following topics:

-   [Configuring views](#configuring-views)
-   [Creating new views](#creating-new-views)

## Configuring views

Views in Rerun are configured by [Blueprints](../../reference/viewer/blueprint.md). We will now use blueprints to adjust
both an individual entity as well as the contents of a space view itself.

### Adjusting entity properties

First, click to select the entity named `points` in the `/ (Spatial)` view in the Blueprint panel. Now, look and the
selection panel -- in addition to the information about the data associated with that entity, you will see a "Blueprint"
section.

Try toggling "visible" on and off and you will see that the points disappear and reappear. Next, click the control
labeled "visible history" and drag it to the right to increase the value. As you drag farther you will see more points
show up in the view. This is making historical points, from farther back in time visible within the time point of this
view. Because the points are logged in stationary 3D space, aggregating them here gives us a more complete view of the
car. Leave the visible history with a value of 50.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough8_history/9c6a01f4dd2059641d92d121f8f2772203c56cfa/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough8_history/9c6a01f4dd2059641d92d121f8f2772203c56cfa/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough8_history/9c6a01f4dd2059641d92d121f8f2772203c56cfa/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough8_history/9c6a01f4dd2059641d92d121f8f2772203c56cfa/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough8_history/9c6a01f4dd2059641d92d121f8f2772203c56cfa/full.png" alt="viewer walkthrough adjusting visible history screenshot">
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
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough9_add_remove/e22b231be49391998d6e6ef005b2dad0a85d2adf/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough9_add_remove/e22b231be49391998d6e6ef005b2dad0a85d2adf/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough9_add_remove/e22b231be49391998d6e6ef005b2dad0a85d2adf/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough9_add_remove/e22b231be49391998d6e6ef005b2dad0a85d2adf/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough9_add_remove/e22b231be49391998d6e6ef005b2dad0a85d2adf/full.png" alt="viewer walkthrough modifying contents of a space view screenshot">
</picture>

## Creating new views

New views & view containers (grid, vertical, etc.) can be created using the "+" button at the top of the Blueprint panel or
from the selection panel when selecting a container.

After creating a view you usually want to proceed to editing its origin and query (which entities are shown) in the selection panel.

Your view layout might be feeling a little cluttered now. You can quickly hide views you're
not using from the blueprint panel by hovering over the view and then clicking the icon that looks like an eye. Go ahead
and hide the `image` and `avg_reproj_err` views, and collapse the expanded timeline panel using the button in the upper
right corner. Note that even with the timeline collapsed you still have access to timeline controls, including a slider.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough11_toggle_vis/28d2720b63fbb2f3d3def0f37962f1ace3674085/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough11_toggle_vis/28d2720b63fbb2f3d3def0f37962f1ace3674085/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough11_toggle_vis/28d2720b63fbb2f3d3def0f37962f1ace3674085/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough11_toggle_vis/28d2720b63fbb2f3d3def0f37962f1ace3674085/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough11_toggle_vis/28d2720b63fbb2f3d3def0f37962f1ace3674085/full.png" alt="viewer walkthrough toggle visibility screenshot">
</picture>

### Reusing what you've learned

Finally, use what we covered in the previous section to change the contents of this view. Select the new `camera` view,
then choose "Add/remove entities." Remove the 2D "keypoints" and add in the 3D "points." Note that these points do not
have visible history turned on -- that's because the blueprint is part of the view and not part of the entity.
Select the points within this view by clicking on them in the blueprint or the view itself, and then give them visible
history as well. When you are done, your view should look like this:

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/viewer_walkthrough12_cameraview/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/viewer_walkthrough12_cameraview/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/viewer_walkthrough12_cameraview/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/viewer_walkthrough12_cameraview/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb/1200w.png">
  <img src="https://static.rerun.io/viewer_walkthrough12_cameraview/3813b97238a2e3a8f5503ac3a408a8c9d0f5dadb/full.png" alt="viewer walkthrough camera view screenshot">
</picture>

Now move the slider back and forth and see what happens. Even though they are both views of the same camera and point
entities, they behave quite differently. On the top the camera moves relative to the car, while on the bottom the car
moves relative to the camera. This is because the new views have _different_ space roots, and Rerun uses the transform
system to transform or project all data into the space root for the given view.

## Conclusion

That brings us to the end of this walkthrough. To recap, you have learned how to:

-   Configure entity blueprint properties.
-   Add and remove entities from views.
-   Create and configure new views.
-   And some basics of how transforms work.

Again, if you ran into any issues following this guide, please don't hesitate to [open an issue](https://github.com/rerun-io/rerun/issues/new/choose).

### Up next

To get started with writing a program to logging data with the Rerun SDK see the [getting started guides](../quick-start).

To see and explore other data, you can check out the [examples](/examples).

For deeper context on the ideas covered here, consult the [Concept overview](../../concepts.md).
