---
title: Clear out data using tombstones
order: 600
description: How to log data that isn't valid for the whole recording
---
In order to create coherent views of streaming data, the Rerun Viewer shows the latest values for each visible entity at the current timepoint. But some data may not be valid for the entire recording even if there are no updated values. How do you tell Rerun that something you've logged should no longer be shown?

## Log entities as cleared
The most straight forward option is to explicitly log that an entity has been cleared. Rerun allows you to do this by logging a special `Clear` to any path. The timepoint at which the `Clear` is logged is the time point after which that entity will no longer be visible in your views.

For example, if you have an object tracking application, your code might look something like this:
```python
…
for frame in sensors.read():
    # Associate the following logs with `frame == frame.id`
    rr.set_time("frame", sequence=frame.id)
    # Do the actual tracking update
    tracker.update(frame)
    if tracker.is_lost:
        # Clear everything on or below `tracked/{tracker.id}`
        # and that happened on or before `frame == frame.id`
        rr.log(f"tracked/{tracker.id}", rr.Clear(recursive=True))
    else:
        # Log data to the main entity and a child entity
        rr.log(f"tracked/{tracker.id}", rr.Rect2D(tracker.bounds))
        rr.log(f"tracked/{tracker.id}/cm", rr.Point2D(tracker.cm))

```
## Clarify data meaning
In some cases, the best approach may be to rethink how you log data to better express what is actually happening. Take the following example where update frequencies don't match:

```python
…
for frame in sensors.read():
    # Associate the following logs with `frame = frame.id`
    rr.set_time("frame", sequence=frame.id)
    # Log every image that comes in
    rr.log("input/image", rr.Image(frame.image))
    if frame.id % 10 == 0:
        # Run detection every 10 frames
        detection = detector.detect(frame)

        # Woops! These detections will not update at the
        # same frequency as the input data and thus look strange
        rr.log("input/detections", rr.Rect2D(detection.bounds))
```
You could fix this example by logging `rr.Clear`, but in this case it makes more sense to change what you log to better express what is happening. Re-logging the image to another namespace on only the frames where the detection runs makes it explicit which frame was used as the input to the detector. This will create a second view in the Viewer that always allows you to see the frame that was used for the current detection input.

Here is an example fix:
```python
class Detector:
    …
    def detect(self, frame):
        downscaled = self.downscale(frame.image)
        # Log the downscaled image
        rr.log("detections/source", rr.Image(downscaled))
        result = self.model(downscaled)
        detection = self.post_process(result)
        # Log the detections together with the downscaled image
        # Image and detections will update at the same frequency
        rr.log("downscaled/detections", rr.Rect2D(detection.bounds))
        return detection
…
for frame in sensors.read():
    # Associate the following logs with `frame = frame.id`
    rr.set_time("frame", sequence=frame.id)
    # Log every image that comes in
    rr.log("input/image", rr.Image(frame.image))
    if frame.id % 10 == 0:
        # Run detection every 10 frames
        # Logging of detections now happens inside the detector
        detected = detector.detect(frame)
```

## Log data with spans instead of timepoints
In some cases you already know how long a piece of data will be valid at the time of logging. Rerun does **not yet support** associating logged data with spans like `(from_timepoint, to_timepoint)` or `(timepoint, time-to-live)`.

Follow the issue [here](https://github.com/rerun-io/rerun/issues/3008).

### Workaround by manually clearing entities
For now the best workaround is to manually clear data when it is no longer valid.
```python
# Associate the following data with `start_time` on the `time` timeline
rr.set_time("time", duration=start_time)
# Log the data as usual
rr.log("short_lived", rr.Tensor(one_second_tensor))
# Associate the following clear with `start_time + 1.0` on the `time` timeline
rr.set_time("time", duration=start_time + 1.0)
rr.log("short_lived", rr.Clear(recursive=False))  # or `rr.Clear.flat()`
# Set the time back so other data isn't accidentally logged in the future.
rr.set_time("time", duration=start_time)
```
