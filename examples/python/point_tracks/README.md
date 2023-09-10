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

In this recipe we will show how to combine points and line segments to create interactive point tracks in Rerun. A real-world implementation of the same idea can be found in the [TAPIR](/examples/paper-visualizations/tapir) example.

```python
import numpy as np
import rerun as rr

# define some parameters to generate the data
num_tracks = 50
duration = 3.0
max_x = 4 * np.pi
num_steps = int(duration * 30)

# generate multiple randomly offset sine waves as point tracks
x_values = np.linspace(0.0, max_x, num_steps)
time_steps = np.linspace(0.0, duration, num_steps)
base_point_track = np.stack((x_values, np.cos(x_values)), axis=-1)
offsets = np.random.randn(num_tracks, 1, 2)
tracks = offsets + base_point_track[None]

# start Rerun prior to logging anything
rr.init("Point Tracks", spawn=True)

# log the points so we can visualize the tracks
for i, time_step in enumerate(time_steps):
    rr.set_time_seconds("time", time_step)
    rr.log_points("point_tracks/points", tracks[:, i])
```
TODO(roym899) GIF of current state

We can see the points, but it is difficult to see the exact path they follow. To add a history we can log line segments between points adjacent in time. To do this we need to log the line segments between the points in the current time step and the previous time step. We can do this by stacking the points in the current time step and the previous time step and then reshaping the array to get the line segments in the format Rerun expects. Note that we need to skip the first time step since there is no previous time step to log the line segments to.
```python
for i, time_step in enumerate(time_steps):
    ... # log points as before
    if i > 1:
        segments = np.stack((tracks[:, i - 1], tracks[:, i]), axis=1)
        rr.log_line_segments("point_tracks/lines", segments.reshape(-1, 2))
```
By default, we only see the a single segment between the current point and the previous point. However, by adjusting the visible history within the Rerun viewer the length of the point tracks can easily be adjusted.

TODO(roym899) GIF of current state with sliding history

It is common for points to become temporarily occluded in videos. To reflect this we add another array that contains the boolean visibility of each point at each time step and skip logging the point and track when the point is not visible. Note that the track should only be logged when the point is visible in both the current and previous time step.
```python
# generate visibility mask (here we hide a stripe around 2 * np.pi)
visibilities = np.abs(tracks[:, :, 0] - 2 * np.pi) > 0.5

for i, time_step in enumerate(time_steps):
    mask = visibilities[:, i]
    rr.set_time_seconds("time", time_step)
    rr.log_points("point_tracks/points", tracks[mask, i])

    if i > 1:
        seg_mask = visibilities[:, i] * visibilities[:, i - 1]
        segments = np.stack((tracks[seg_mask, i - 1], tracks[seg_mask, i]), axis=1)
        rr.log_line_segments("point_tracks/lines", segments.reshape(-1, 2))
```
TODO(roym899) GIF of current state

This will stop updating the point position when it is not visible, however, if no point is visible the points will stay at the last logged position. An easy way to avoid this, is to always call `rr.log_cleared` prior to logging the points. This way, we always start from a clean slate at each time step.

```python
for i, time_step in enumerate(time_steps):
    ... # compute mask as before
    rr.log_cleared("point_tracks/points")
    rr.log_points("point_tracks/points", tracks[mask, i])

    if i > 1:
        ... # compute segments as before
        rr.log_cleared("point_tracks/lines")
        rr.log_line_segments("point_tracks/lines", segments.reshape(-1, 2))
```

<!-- Add this once log_line_segments supports multiple colors (otherwise need ugly workaround)

To keep track of each point it can be useful to assign a unique color to each point track. In this example, we use matplotlib's colormap to generate a color for each point track based on the initial point position. We then use the color to set the color of the point and the line segment. Note that we use the same color for the point and the line segment to make it clear that they belong to the same point track.
```python

```
TODO(roym899) GIF of current state -->

Putting everything together and wrapping the logic in functions we get the following code:
```python
import numpy as np
import rerun as rr


def log_point_tracks(entity_path, tracks, visibilities, time_steps):
    """Log multiple point tracks.

    Args:
        entity_path:
            Entity path to which point tracks are logged.
        tracks: The point tracks to log. Shape (num_tracks, num_time_steps, 2 or 3).
        visibilities:
            The visibility of the point tracks. Shape (num_tracks, num_time_steps).
        time_steps:
            The time steps for each point in the point tracks. Shape (num_time_steps,).
    """
    for i, time_step in enumerate(time_steps):
        mask = visibilities[:, i]
        rr.set_time_seconds("time", time_step)
        rr.log_cleared(entity_path + "/points")
        rr.log_points(entity_path + "/points", tracks[mask, i])

        if i > 1:
            seg_mask = visibilities[:, i] * visibilities[:, i - 1]
            segments = np.stack((tracks[seg_mask, i - 1], tracks[seg_mask, i]), axis=1)
            rr.log_cleared(entity_path + "/lines")
            rr.log_line_segments(entity_path + "/lines", segments.reshape(-1, 2))


def generate_data():
    """Generate point track data for this example."""
    num_tracks = 50
    duration = 3.0
    max_x = 4 * np.pi
    num_steps = int(duration * 30)

    x_values = np.linspace(0.0, max_x, num_steps)
    time_steps = np.linspace(0.0, duration, num_steps)
    base_point_track = np.stack((x_values, np.cos(x_values)), axis=-1)
    offsets = np.random.randn(num_tracks, 1, 2)
    tracks = offsets + base_point_track[None]
    visibilities = np.abs(tracks[:, :, 0] - 2 * np.pi) > 0.5

    return tracks, visibilities, time_steps


rr.init("Point Tracks", spawn=True)
tracks, visibilities, time_steps = generate_data()
log_point_tracks("point_tracks", tracks, visibilities, time_steps)
```
Check out the [TAPIR](/examples/paper-visualizations/tapir) example to see how to use this idea to log estimated point tracks from a real-world video.
