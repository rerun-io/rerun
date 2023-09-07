---
title: Point Tracks
thumbnail: https://static.rerun.io/92a1f80b5cf2cd2c04a10d8ced35849da8f1c0ed_minimal_480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/92a1f80b5cf2cd2c04a10d8ced35849da8f1c0ed_minimal_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/d78037f2306ed02505859adbae9f72d4ab2945d1_minimal_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/cf926c580c8ca8b39fd844f6adf4b19972b5111e_minimal_1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/8f03efd9e918f43b5e6d9257d0f1a3cb962b3889_minimal_1200w.png">
  <img src="https://static.rerun.io/0e47ac513ab25d56cf2b493128097d499a07e5e8_minimal_full.png" alt="Minimal example screenshot">
</picture>

In this recipe we will show how to combine points and line segments to create interactive point tracks in Rerun. A bigger example building on the same idea can be found in the [TAPIR]() example.

```python
# TODO(roym899) start by generating points and logging just the points
```
TODO(roym899) GIF of current state

```python
# TODO(roym899) add history with line segments
```
TODO(roym899) GIF of current state with sliding history

It is common for points to become temporarily occluded in videos. To reflect this we add another array that contains the boolean visibility of each point at each time step and skip logging the point and track when the point is not visible. Note that the track should only be logged when the point is visible in both the current and previous time step.
```python
# TODO(roym899) add cleared
```
TODO(roym899) GIF of current state

This will stop updating the point position when it is not visible, however, the point stays at the last known position. Using `rr.log_cleared` we can clear the points at each frame prior to logging the new points. This will make the points disappear when they are not visible.

```python
# TODO(roym899) add cleared
```
TODO(roym899) GIF of current state


Putting everything together and wrapping the logic in functions we get the following code:
```python
import numpy as np
import rerun as rr

def log_point_track(entity_path, point_track, point_visibility, time_steps):
    """Log a single point track.

    Args:
      point_track: The point track to log. Shape (num_time_steps, 2 or 3).
      point_visibility: The visibility of each point in the point track. Shape (num_time_steps,).
      time_steps: The time steps for each point in the point tracks. Shape (num_time_steps,).
    """
    for i, point_track in enumerate(point_tracks):
      for  in zip(point_track, time_steps):
          print(f"  {point}")

def log_point_tracks(entity_path, point_tracks, point_visibilities, time_steps):
    """Log multiple point tracks.

    Args:
      point_track: The point track to log. Shape (num_tracks, num_time_steps, 2 or 3).
      time_steps: The time steps for each point in the point tracks. Shape (num_time_steps,).
    """
    for point_track, point_visibility in zip(point_track, time_steps):
        log_point_track(point_track, point_visibility, time_steps)

def generate_data():
    """Generate data for the example."""
    num_tracks = 10
    num_time_steps = 100
    points_dim = 2
    # TODO(roym899) generate some nice with easy to understand visibility
    return point_tracks, point_visibilities, time_steps

rr.init("Point Tracks", spawn=True)
point_tracks, point_visibilities, time_steps = generate_data()
log_point_tracks("point_tracks", point_tracks, point_visibilities, time_steps)
```
Check out the [TAPIR]() example to see how to use this idea to log estimated point tracks from a real-world video.
