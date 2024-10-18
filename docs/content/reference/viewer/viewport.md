---
title: Viewport
order: 4
---

The viewport is a flexible area where you can arrange your Views, sometimes also referred to as Space Views.
You can grab the title of any View to dock it to different parts of the viewport or to form tabs.

View controls
-------------

<picture>
  <img src="https://static.rerun.io/view-controls/e911cec51fcf840e014340b3cb135b7faeb2e8b6/full.png" alt="">
</picture>


Clicking on the title of a View has the same effect as selecting it in the [Blueprint view](blueprint.md)
and will show additional information & settings in the [Selection view](selection.md) or other means.

For more information on how to navigate a specific View, hover its help icon at the top right corner.

The maximize button makes a single View fill the entire viewport.
Only one view can be maximized at a time.


View Classes
---------------------------
Rerun distinguishes various different built-in Views classes.
The class of a view determines which visualizers are available and thus what data can be displayed, how it will be shown and the way they can be interacted with.

There are a variety of classes to choose from, for an overview check the blueprint type documentation on [Views](../../reference/types/views.md).
Which class a View uses is always determined upon creation.

To learn more about the _internals_ of how Space View classes work, check the [guide on Viewer extensions](../../howto/extend.md).
