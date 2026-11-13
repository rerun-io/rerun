---
title: Control the 3D camera
order: 100
description: Set the 3D camera position, target, and up axis, or follow an entity
---

In a 3D view, the camera that you look through is called the _eye_.
You can set the eye from code with the `eye_controls` parameter of [`Spatial3DView`](../../reference/types/views/spatial3d_view.md), which takes an `EyeControls3D` blueprint archetype (see [`eye_controls`](../../reference/types/views/spatial3d_view.md#eye_controls)).

## Set the eye position and look target

```python
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_eye_controls", spawn=True)

rr.log("world/points", rr.Points3D([[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]], radii=0.05))

rr.send_blueprint(
    rrb.Spatial3DView(
        origin="/world",
        eye_controls=rrb.EyeControls3D(
            position=[4.0, -4.0, 3.0],
            look_target=[0.0, 0.0, 0.0],
            eye_up=[0.0, 0.0, 1.0],
        ),
    )
)
```

`EyeControls3D` has these fields:

| Field             | Meaning                                                                                               |
|-------------------|-------------------------------------------------------------------------------------------------------|
| `kind`            | `rrb.Eye3DKind.Orbital` (default) or `rrb.Eye3DKind.FirstPerson`.                                     |
| `position`        | The position of the eye.                                                                              |
| `look_target`     | The point that the eye looks at. For an orbital eye, this is also the point it orbits around.         |
| `eye_up`          | The up axis of the eye, in world space. Defaults to the up axis of the scene, or +Z if there is none. |
| `speed`           | Translation speed when you move with the WASDQE keys.                                                 |
| `tracking_entity` | An entity that the eye follows. See below.                                                            |
| `spin_speed`      | Spin the eye around the up axis at this speed. Defaults to 0 (no spinning).                           |

The scene up axis comes from [`ViewCoordinates`](../../reference/types/archetypes/view_coordinates.md).
For example, log `rr.ViewCoordinates.RIGHT_HAND_Z_UP` as static data on the view's origin entity to make +Z the up axis.

The blueprint sets the eye when the Viewer receives it.
After that, the user can move the eye freely.
Send the blueprint again to move the eye back.

## Orbital or first-person controls

- `Orbital`: the eye rotates around `look_target`.
  Scroll zooms toward and away from it.
- `FirstPerson`: the eye rotates around its own position, like in a first-person game.
  Use WASD and QE to move.

```python
rrb.Spatial3DView(
    origin="/world",
    eye_controls=rrb.EyeControls3D(kind=rrb.Eye3DKind.FirstPerson, speed=20.0),
)
```

You can also switch between the two in the Viewer: select the view and change the eye kind in the selection panel.

## Follow an entity

Set `tracking_entity` to make the eye follow an entity as it moves:

```python
rrb.Spatial3DView(
    origin="/world",
    eye_controls=rrb.EyeControls3D(tracking_entity="/world/robot"),
)
```

If the tracked entity is a camera (an entity with a [`Pinhole`](../../reference/types/archetypes/pinhole.md)), the eye takes over the pose of that camera, so you see the scene through it.

In the Viewer, you can start tracking an entity with Alt + double-click on it, or with "Set as eye tracked" in its context menu.
Double-click an entity to focus on it, and double-click the background to reset the eye.

## Limitations

There is currently no way to:

- lock the eye, so that the user cannot move it,
- limit how far the user can zoom in or out.

See [missing features](../../development/roadmap.md#missing-features).
