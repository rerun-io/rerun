---
title: Viewport
order: 4
---

The viewport is a flexible area where you can arrange your views:
You can grab the title of any view to dock it to different parts of the viewport or to form tabs.

## View controls

<picture>
  <img src="https://static.rerun.io/view-controls/e911cec51fcf840e014340b3cb135b7faeb2e8b6/full.png" alt="">
</picture>


Clicking on the title of a view has the same effect as selecting it in the [blueprint panel](blueprints.md)
and will show additional information and settings in the [selection panel](selection.md).

For more information on how to navigate within a specific view, hover its help icon in the top right corner.

The maximize button makes a single view fill the entire viewport.
Only one view can be maximized at a time.


## View classes

Rerun includes multiple view classes, each dedicated to a specific type of visualization; for example, a 3D scene or a timeseries plot.
See the [views reference page](../types/views.md) for a list of available view classes.

The view class, which is specified upon creation, determines which entities it can display, how it displays them, and the way they can be interacted with.
Views can be created both from viewer and from code (see [Configure the Viewer](../../getting-started/configure-the-viewer.md)).

To learn more about the _internals_ of how view classes work, check the [guide on implementing custom views](../../howto/visualization/extend-ui.md).
