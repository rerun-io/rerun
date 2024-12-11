---
title: Migrating from 0.20 to 0.21
order: 989
---

### File compatibility
We've changed how tensors are encoded in .rrd files, so tensors will no longer load from older .rrd files ([#8376](https://github.com/rerun-io/rerun/pull/8376)).

### Near clip plane for `Spatial2D` views now defaults to `0.1` in 3D scene units.

Previously, the clip plane was set an arbitrary value that worked reasonably for
cameras with large focal lengths, but become problematic for cameras with smaller
focal length values. This value is now normalized based on the focal length of
the camera.

If the default value of `0.1` is still too large for your use-case it can be configured
using the new `near_clip_plane` of the `VisualBounds2D` blueprint property, either
through the UI, or through the SDK in Python:
```python
rr.send_blueprint(
    rrb.Spatial2DView(
        origin="world/cam",
        contents="/**",
        visual_bounds=rrb.VisualBounds2D(
            near_clip_plane=0.01,
        ),
    )
)
```


### Types and fields got renamed from `.*space_view.*`/`.*SpaceView.*` to `.*view.*`/`.*View.*`

Various types and fields got changed to refer to "views" rather than "space views".
This exclusively affects the Python blueprint sdk:

#### Field/argument changes:
* `ViewportBlueprint(...auto_views=...)` -> `ViewportBlueprint(...auto_views=...)`
* `Blueprint(...auto_views=...)` -> `Blueprint(...auto_views=...)`

#### Type changes

##### Utilities

* `SpaceView` -> `View`

##### Archetypes

* `SpaceViewBlueprint` -> `ViewBlueprint`
* `SpaceViewContents` -> `ViewContents`

##### Components

* `AutoSpaceViews` -> `AutoViews`
* `SpaceViewClass` -> `ViewClass`
* `SpaceViewOrigin` -> `ViewOrigin`
* `SpaceViewMaximized` -> `ViewMaximized`
