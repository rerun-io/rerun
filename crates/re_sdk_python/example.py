"""
Shows how to use the rerun SDK.
"""

from dataclasses import dataclass
import math
from typing import List, Sequence, Tuple

import argparse
import cv2
import numpy as np

import rerun_sdk as rerun


class DummyCar:
    """Class representing a dummy car for generating dummy data to log."""

    def __init__(self, center: Tuple[int, int], size: Tuple[int, int], distance_mm: float):
        self.center = np.array(center)
        self.size = np.array(size)
        self.distance_mm = distance_mm

    @property
    def min(self) -> np.array:
        return self.center - self.size / 2

    @property
    def max(self) -> np.array:
        return self.center + self.size / 2

    def drive_one_step(self):
        self.center[0] += 5
        self.distance_mm -= 1

    def draw_depth(self, depth_image_mm: np.array) -> np.array:
        cv2.rectangle(depth_image_mm, self.min.astype(int), self.max.astype(int),
                      self.distance_mm, cv2.FILLED)
        return depth_image_mm


@dataclass
class SampleFrame:
    """Holds data for a single frame of data."""
    frame_idx: int
    depth_image_mm: np.array
    car_bbox: Tuple[np.array, np.array]


def generate_dummy_data(num_frames: int) -> Sequence[SampleFrame]:
    """This function generates dummy data to log."""
    # Generate some fake data
    im_w = 480
    im_h = 270

    # Pre-generate image containing the x and y coordinates per pixel
    _, yv = np.meshgrid(np.arange(0, im_w), np.arange(0, im_h))

    # Background image as a simple slanted plane
    depth_background_mm = 1000.0 / (0.1 + yv/im_h)

    # Generate `num_frames` sample data
    car = DummyCar(center=(140, 100), size=(200, 100), distance_mm=4000)
    samples = []  # type: List[SampleFrame]
    for i in range(num_frames):
        depth_image_mm = car.draw_depth(depth_background_mm.copy())
        sample = SampleFrame(frame_idx=i,
                             depth_image_mm=depth_image_mm,
                             car_bbox=(car.min, car.size))
        samples.append(sample)
        car.drive_one_step()

    return samples


def log_dummy_data(args):
    NUM_FRAMES = 40

    for sample in generate_dummy_data(num_frames=NUM_FRAMES):
        # This will assign logged objects a "time source" called `frame_nr`.
        # In the viewer you can select how to view objects - by frame_nr or the built-in `log_time`.
        rerun.set_time_sequence("frame_nr", sample.frame_idx)

        # The depth image is in millimeters, so we set meter=1000
        rerun.log_depth_image("depth", sample.depth_image_mmmage, meter=1000)

        ((car_x, car_y), (car_w, car_h)) = sample.car_bbox
        rerun.log_bbox("bbox", [car_x, car_y], [car_w, car_h], "A car")

    if False:
        depth_img = cv2.imread('depth_image.pgm', cv2.IMREAD_UNCHANGED)
        rerun.log_depth_image("depth_image", depth_img, meter=10_000)

    if True:
        image = cv2.imread('crates/re_viewer/data/logo_dark_mode.png',
                           cv2.IMREAD_UNCHANGED)
        rerun.log_image("logo", image, space="rgb")

    if True:
        positions = []
        for i in range(1000):
            angle = 6.28 * i / 64
            r = 1.0
            x = r * math.cos(angle) + 18.0
            y = r * math.sin(angle) + 16.0
            z = i / 64.0
            positions.append([x, y, z])
        positions = np.array(positions)

        # Same color for all points in this case, but you can also have a different color for each point:
        rgba = [200, 0, 100, 200]
        colors = np.array([rgba])

        rerun.log_points("point3d", positions, colors)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs rich data using the Rerun SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    args = parser.parse_args()

    if args.connect:
        # Send logging data to separate `rerun` process.
        # You can ommit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    print(rerun.info())

    log_dummy_data(args)

    if not args.connect:
        rerun.show()
