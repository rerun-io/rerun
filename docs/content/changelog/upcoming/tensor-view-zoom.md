---
title: "Pan and zoom in the tensor view"
hidden: true
type: misc
---

The [tensor view](../reference/types/views/tensor_view.md) now supports pan and zoom, with the same controls as the 2D view: drag or scroll to pan, zoom with the scroll modifier or a pinch, and double-click to reset.
The visible region is stored in the new `visual_bounds` view property, so it can also be set from the SDK.
It is reset to the whole slice when the dimensions shown as width or height change, or when the view fit changes.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/ea60f9e014adbbbcbec191d62bd096cd9599a7a4_tensor_view_zoom_viewer.mp4" type="video/mp4" />
</video>
