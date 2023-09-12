---
title: Point Tracks
thumbnail: https://static.rerun.io/0d2e95315a9eb546cf6eecbc2642a044d044141a_point_tracks_recipe_480w.png
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/c7f24fbacc5b61e53c6ff9367d464e99fb46ed06_point_tracks_recipe_header_480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/4a12cb85034faa68a7cac2c8bacbc9c372f20c03_point_tracks_recipe_header_768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/bf309584ad5490eb4be216fc37bc5e071a00f541_point_tracks_recipe_header_1024w.png">
  <img src="https://static.rerun.io/49b8447fd4aa2b0544aeed978b3f3c0bd33b93c5_point_tracks_recipe_header_full.png" alt="">
</picture>


In this recipe we will show how to combine points and line segments to create interactive point tracks (with temporary occlusion) like the ones shown above in Rerun. A real-world implementation of the same idea can be found in the [TAPIR](/examples/paper-visualizations/tapir) example.

Let's start by generating some point tracks. For this example, we will generate multiple randomly offset sine waves as our point tracks. We also immediately visualize these point tracks by logging them to Rerun.
```python
import numpy as np
import rerun as rr
import rerun.experimental as rr2

# define some parameters to generate the data
num_tracks = 20
duration = 3.0
max_x = 4 * np.pi
num_steps = int(duration * 60)

# generate multiple randomly offset sine waves as point tracks
x_values = np.linspace(0.0, max_x, num_steps)
time_steps = np.linspace(0.0, duration, num_steps)
base_point_track = np.stack((x_values, np.cos(x_values)), axis=-1)
offsets = np.random.randn(num_tracks, 1, 2)
tracks = offsets + base_point_track[None]

# start Rerun prior to logging anything
rr.init("rerun_example_point_tracks", spawn=True)

# log the points so we can visualize the tracks
for i, time_step in enumerate(time_steps):
    rr.set_time_seconds("time", time_step)
    rr2.log("point_tracks/points", rr2.Points2D(tracks[:, i]))
```
TODO(roym899) GIF of current state

We can see the points, but it is difficult to see the exact path they follow. To add a history we can log line segments between points adjacent in time. To do this we need to log the line segments between the points in the current time step and the previous time step. We can do this by stacking the points in the current time step and the previous time step and then reshaping the array to get the line segments in the format Rerun expects. Note that we need to skip the first time step since there is no previous time step to log the line segments to.
```python
for i, time_step in enumerate(time_steps):
    … # log points as before
    if i > 1:
        segments = np.stack((tracks[:, i - 1], tracks[:, i]), axis=1)
        rr2.log("point_tracks/lines", rr2.LineStrips2D(segments))
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
    rr2.log("point_tracks/points", rr2.Points2D(tracks[mask, i]))

    if i > 1:
        seg_mask = visibilities[:, i] * visibilities[:, i - 1]
        segments = np.stack((tracks[seg_mask, i - 1], tracks[seg_mask, i]), axis=1)
        rr2.log("point_tracks/lines", rr2.LineStrips2D(segments))
```

This will stop updating the point position when it is not visible, however, if no point is visible, the points will stay at the last logged position. An easy way to avoid this, is to always call `rr.log_cleared` prior to logging the points. This way, we always start from a clean slate at each time step.

```python
for i, time_step in enumerate(time_steps):
    … # compute mask as before
    rr.log_cleared("point_tracks/points")
    rr2.log("point_tracks/points", rr2.Points2D(tracks[mask, i]))

    if i > 1:
        … # compute segments as before
        rr.log_cleared("point_tracks/lines")
        rr2.log("point_tracks/lines", rr2.LineStrips2D(segments))
```

TODO(roym899) GIF of current state

To keep track of each point it can be useful to assign a unique color to each point track. To do so we log the track id as the class id of each point and line segment. We also log an annotation context to assign a unique color to each track. Note that we set the `timeless` flag to `True` when logging the annotation context, since a track's color should not change over time.

```python
# assign random color to each track
track_ids = np.arange(0, len(tracks))
annotation_context = [(i, None, np.random.rand(3)) for i in track_ids]
rr2.log(entity_path, rr2.AnnotationContext(annotation_context), timeless=True)

for i, time_step in enumerate(time_steps):
    … # as before
    rr2.log(
        "point_tracks/points",
        rr2.Points2D(tracks[mask, i], class_ids=track_ids[mask]),
    )

    if i > 1:
        … # as before
        rr2.log(
            "point_tracks/lines",
            rr2.LineStrips2D(segments, class_ids=track_ids[seg_mask]),
        )
```

TODO(roym899) GIF of current state

Putting everything together and wrapping the logic in functions we get the following code:
```python
import numpy as np
import rerun as rr
import rerun.experimental as rr2


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
    # assign random color to each track
    track_ids = np.arange(0, len(tracks))
    annotation_context = [(i, None, np.random.rand(3)) for i in track_ids]
    rr2.log(entity_path, rr2.AnnotationContext(annotation_context), timeless=True)

    for i, time_step in enumerate(time_steps):
        mask = visibilities[:, i]
        rr.set_time_seconds("time", time_step)
        rr.log_cleared(entity_path + "/points")
        rr2.log(
            entity_path + "/points",
            rr2.Points2D(tracks[mask, i], class_ids=track_ids[mask]),
        )

        if i >= 1:
            seg_mask = visibilities[:, i] * visibilities[:, i - 1]
            segments = np.stack((tracks[seg_mask, i - 1], tracks[seg_mask, i]), axis=1)
            rr.log_cleared(entity_path + "/lines")
            rr2.log(
                entity_path + "/lines",
                rr2.LineStrips2D(segments, class_ids=track_ids[seg_mask]),
            )


def generate_data():
    """Generate point track data for this example."""
    num_tracks = 20
    duration = 3.0
    max_x = 4 * np.pi
    num_steps = int(duration * 60)

    x_values = np.linspace(0.0, max_x, num_steps)
    time_steps = np.linspace(0.0, duration, num_steps)
    base_point_track = np.stack((x_values, np.cos(x_values)), axis=-1)
    offsets = np.random.randn(num_tracks, 1, 2)
    tracks = offsets + base_point_track[None]
    visibilities = np.abs(tracks[:, :, 0] - 2 * np.pi) > 0.5  # hide strip in middle

    return tracks, visibilities, time_steps


rr.init("rerun_example_point_tracks", spawn=True)
tracks, visibilities, time_steps = generate_data()
log_point_tracks("point_tracks", tracks, visibilities, time_steps)
```
Check out the [TAPIR](/examples/paper-visualizations/tapir) example to see how to use this idea to log estimated point tracks from a real-world video.
