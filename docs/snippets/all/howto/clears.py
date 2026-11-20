"""Clear stale entities, and log derived data together with its input."""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass

import numpy as np

import rerun as rr

rr.init("rerun_example_clears", spawn=True)


# Synthetic stand-ins for a sensor, an object tracker, and an object detector.
@dataclass
class Frame:
    id: int
    image: np.ndarray


@dataclass
class Detection:
    bounds: np.ndarray


class Sensors:
    def read(self) -> Iterator[Frame]:
        for i in range(20):
            yield Frame(
                id=i, image=np.full((24, 32, 3), i * 10, dtype=np.uint8)
            )


class Tracker:
    id = 0

    def __init__(self) -> None:
        self.bounds = np.zeros(4, dtype=np.float32)
        self.cm = np.zeros(2, dtype=np.float32)
        self.is_lost = False

    def update(self, frame: Frame) -> None:
        self.is_lost = frame.id >= 12
        self.bounds = np.array(
            [frame.id, 4, frame.id + 8, 12], dtype=np.float32
        )
        self.cm = np.array([frame.id + 4, 8], dtype=np.float32)


class DetectorModel:
    def downscale(self, image: np.ndarray) -> np.ndarray:
        return image[::2, ::2]

    def model(self, image: np.ndarray) -> np.ndarray:
        return np.array([2, 2, 8, 6], dtype=np.float32)

    def post_process(self, result: np.ndarray) -> Detection:
        return Detection(bounds=result)

    def detect(self, frame: Frame) -> Detection:
        return self.post_process(self.model(self.downscale(frame.image)))


sensors = Sensors()
tracker = Tracker()
detector = DetectorModel()

# region: tracker
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
        rr.log(
            f"tracked/{tracker.id}",
            rr.Boxes2D(array=tracker.bounds, array_format=rr.Box2DFormat.XYXY),
        )
        rr.log(f"tracked/{tracker.id}/cm", rr.Points2D(tracker.cm))
# endregion: tracker

# region: mismatched_frequency
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
        rr.log(
            "input/detections",
            rr.Boxes2D(
                array=detection.bounds, array_format=rr.Box2DFormat.XYXY
            ),
        )
# endregion: mismatched_frequency


# region: detector_fix
class Detector(DetectorModel):
    def detect(self, frame: Frame) -> Detection:
        downscaled = self.downscale(frame.image)
        # Log the downscaled image, with the detections as a child entity
        rr.log("detections/source", rr.Image(downscaled))
        result = self.model(downscaled)
        detection = self.post_process(result)
        # Log the detections together with the downscaled image
        # Image and detections will update at the same frequency
        rr.log(
            "detections/source/boxes",
            rr.Boxes2D(
                array=detection.bounds, array_format=rr.Box2DFormat.XYXY
            ),
        )
        return detection


detector = Detector()

for frame in sensors.read():
    # Associate the following logs with `frame = frame.id`
    rr.set_time("frame", sequence=frame.id)
    # Log every image that comes in
    rr.log("input/image", rr.Image(frame.image))
    if frame.id % 10 == 0:
        # Run detection every 10 frames
        # Logging of detections now happens inside the detector
        detected = detector.detect(frame)
# endregion: detector_fix

start_time = 0.0
one_second_tensor = np.ones((4, 4), dtype=np.float32)

# region: manual_clear
# Associate the following data with `start_time` on the `time` timeline
rr.set_time("time", duration=start_time)
# Log the data as usual
rr.log("short_lived", rr.Tensor(one_second_tensor))
# Associate the following clear with `start_time + 1.0` on the `time` timeline
rr.set_time("time", duration=start_time + 1.0)
rr.log("short_lived", rr.Clear(recursive=False))  # or `rr.Clear.flat()`
# Set the time back so other data isn't accidentally logged in the future.
rr.set_time("time", duration=start_time)
# endregion: manual_clear
