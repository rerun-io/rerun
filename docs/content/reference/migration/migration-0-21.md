---
title: Migrating from 0.20 to 0.21
order: 989
---

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

