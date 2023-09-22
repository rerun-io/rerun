---
title: Viewport
order: 4
---

The viewport is a flexible area where you can arrange your Space Views:
You can grab the title of any Space View to dock it to different parts of the viewport or to form tabs.

View controls
-------------

<picture>
  <img src="https://static.rerun.io/d93ec977f99173207c57ab790b8e3112131b1bc1_view-controls_full.png" alt="">
</picture>


Clicking on the title of a Space View has the same effect as selecting it in the [Blueprint view](blueprint.md)
and will show additional information & settings in the [Selection view](selection.md) or other means.

For more information on how to navigate a specific Space View, hover its help icon at the top right corner.

The maximize button makes a single Space View fill the entire viewport.
Only one Space view can be maximized at a time.


Categories of Space Views
---------------------------
Rerun distinguishes various categories of Space Views:
* Spatial
  * Generic 2D & 3D data.
* Tensor
  * Tensor view with support for arbitrary dimensionality.
* Text log
  * Text over time.
* Time series plot
  * Scalars over time.
* Bar chart
  * Bar-chart lots made from 1D tensor data.

Which category is used is determined upon creation of a Space View.

[TODO(#1164)](https://github.com/rerun-io/rerun/issues/1164): Allow configuring the category of a space view after its creation.

The kind of Space View determines which Entities it can display, how it displays them and the way they can be interacted with.
