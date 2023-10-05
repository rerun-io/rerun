---
title: Viewport
order: 4
---

The viewport is a flexible area where you can arrange your Space Views:
You can grab the title of any Space View to dock it to different parts of the viewport or to form tabs.

View controls
-------------

<picture>
  <img src="https://static.rerun.io/view-controls/e911cec51fcf840e014340b3cb135b7faeb2e8b6/full.png" alt="">
</picture>


Clicking on the title of a Space View has the same effect as selecting it in the [Blueprint view](blueprint.md)
and will show additional information & settings in the [Selection view](selection.md) or other means.

For more information on how to navigate a specific Space View, hover its help icon at the top right corner.

The maximize button makes a single Space View fill the entire viewport.
Only one Space view can be maximized at a time.


Space View Classes
---------------------------
Rerun distinguishes various Space Views classes:

* 2D
  * General 2D content like images, lines, points, boxes, etc.
* 3D
  * 3D scene with cameras, meshes, points, lines etc.
* Tensor
  * Tensor view with support for arbitrary dimensionality.
* Text log
  * Text over time.
* Text Document
  * Shows a single markdown or raw text document.
* Time series plot
  * Scalars over time.
* Bar chart
  * Bar-chart lots made from 1D tensor data.

Which class is used is determined upon creation of a Space View.

The Space View class determines which Entities it can display, how it displays them and the way they can be interacted with.
To learn more about the _internals_ of how Space View classes work, check the [guide on viewer extensions](../../howto/extend.md).
