"""
Shows how to use the rerun SDK.
"""

import math
from typing import Sequence

import argparse
import cv2
import numpy as np

import rerun_sdk as rerun


def car_bbox(frame_nr):
    """returns a bbox for a car"""
    (x, y) = (40 + frame_nr * 5, 50)
    (w, h) = (200, 100)
    return ((x, y), (w, h))


def generate_depth_image(frame_nr):
    """
    Return a depth image in millimeter units
    """
    ((car_x, car_y), (car_w, car_h)) = car_bbox(frame_nr)
    car_dist = 4000 - frame_nr

    # Generate some fake data:
    w = 480
    h = 270

    depth_image = np.zeros([h, w])
    for y in range(h):
        for x in range(w):
            d = 1000.0 / (0.1 + y/h)
            if car_x <= x <= car_x + car_w and car_y <= y <= car_y + car_h:
                d = car_dist
            depth_image[(y, x)] = d

    return depth_image


def generate_dummy_data(num_frames: int) -> Sequence[np.array]:
    """
    This function generates dummy data to log.
    """

    return [generate_depth_image(i) for i in range(num_frames)]

def log(args):
    NUM_FRAMES = 40
    depth_images = generate_dummy_data(num_frames=NUM_FRAMES)

    for frame_nr in range(NUM_FRAMES):
        # This will assign logged objects a "time source" called `frame_nr`.
        # In the viewer you can select how to view objects - by frame_nr or the built-in `log_time`.
        rerun.set_time_sequence("frame_nr", frame_nr)

        # The depth image is in millimeters, so we set meter=1000
        depth_image = depth_images[frame_nr]
        rerun.log_depth_image("depth", depth_image, meter=1000)

        ((car_x, car_y), (car_w, car_h)) = car_bbox(frame_nr)
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

    log(args)

    if not args.connect:
        rerun.show()
