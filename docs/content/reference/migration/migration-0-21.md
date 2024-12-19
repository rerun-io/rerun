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


### Blueprint types and fields got renamed from `.*space_view.*`/`.*SpaceView.*` to `.*view.*`/`.*View.*`

Various types and fields got changed to refer to "views" rather than "space views".
This exclusively affects the Python blueprint sdk.

#### Field/argument changes

* `ViewportBlueprint(...auto_space_views=...)` -> `ViewportBlueprint(...auto_views=...)`
* `Blueprint(...auto_space_views=...)` -> `Blueprint(...auto_views=...)`

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


### 3D transform arrow visualization show up less often by default

Previously, the viewer would show 3 arrows for every logged transform if any of the following was true:
* enabled visualizer via `VisualizerOverrides` or ui
* `AxisLength` component is present as well
* there's a pinhole camera at the same path
* no other visualizer would be active by default on the path

For many usecases this led to too many arrows being shown by default.
We therefore removed the last condition - arrows will no longer show by default if they're the only visualizer.
The easiest way to opt-in to transform arrows is to set `AxisLength` (`axis_length` field on the `Transform3D` archetype) on your transforms.

### `DisconnectedSpace` archetype/component deprecated

The `DisconnectedSpace` archetype and `DisconnectedSpace` component have been deprecated.
To achieve the same effect, you can log any of the following "invalid" transforms:
* zeroed 3x3 matrix
* zero scale
* zeroed quaternion
* zero axis on axis-angle rotation

Previously, the `DisconnectedSpace` archetype played a double role by governing view spawn heuristics & being used as a transform placeholder.
This led to a lot of complexity and often broke or caused confusion (see https://github.com/rerun-io/rerun/issues/6817, https://github.com/rerun-io/rerun/issues/4465, https://github.com/rerun-io/rerun/issues/4221).
By now, explicit blueprints offer a better way to express which views should be spawned and what content they should query.
(you can learn more about blueprints [here](https://rerun.io/docs/getting-started/configure-the-viewer/through-code-tutorial)).

`DisconnectedSpace` will be removed in a future release.

### `RotationAxisAngle` with zero rotation axis is no longer treated as identity

Previously, `RotationAxisAngle` with a zero rotation axis was treated as identity.
This is no longer the case, instead it makes the transform invalid in the same way a zeroed transformation matrix does.
