import argparse
import cv2
import math
import numpy as np
import time

import rerun


def log(args):
    if args.connect:
        rerun.connect_remote()
    else:
        rerun.buffer()

    print(rerun.info())

    if False:
        image = cv2.imread('crates/re_viewer/data/logo_dark_mode.png',
                           cv2.IMREAD_UNCHANGED)
        rerun.log_image("logo", image)

    if True:
        depth_img = cv2.imread('depth_image.pgm', cv2.IMREAD_UNCHANGED)
        rerun.log_depth_image("depth", depth_img, meter=10_000)

    if True:
        x, y = 200.0, 50.0
        w, h = 320, 240
        rerun.log_bbox("bbox", [x, y], [w, h], "Label")

    if False:
        for i in range(64):
            angle = 6.28 * i / 64
            r = 20.0
            x = r * math.cos(angle) + 18.0
            y = r * math.sin(angle) + 16.0
            rerun.log_point2d(f"point2d_{i}", x, y)

    if False:
        positions = []
        for i in range(1000):
            angle = 6.28 * i / 64
            r = 1.0
            x = r * math.cos(angle) + 18.0
            y = r * math.sin(angle) + 16.0
            z = i / 64.0
            positions.append([x, y, z])
        positions = np.array(positions)

        # Same for all points, but you can also have a different color for each point:
        colors = np.array([[200, 0, 100, 200]])

        rerun.log_points(f"point3d", positions, colors)

    if not args.connect:
        rerun.show()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs rich data using the Rerurn SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    args = parser.parse_args()

    log(args)
