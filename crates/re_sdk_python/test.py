import argparse
import cv2
import math
import numpy as np
import time

import rerun_sdk as rerun


def log(args):
    if args.connect:
        rerun.connect_remote()
    else:
        rerun.buffer()

    print(rerun.info())

    if False:
        img = cv2.imread('crates/re_viewer/data/logo_dark_mode.png',
                         cv2.IMREAD_UNCHANGED)
        rerun.log_image("logo", img)

    if False:
        for i in range(64):
            angle = 6.28 * i / 64
            r = 20.0
            x = r * math.cos(angle) + 18.0
            y = r * math.sin(angle) + 16.0
            rerun.log_point2d(f"point2d_{i}", x, y)

    if True:
        pos3 = []
        for i in range(1000):
            angle = 6.28 * i / 64
            r = 1.0
            x = r * math.cos(angle) + 18.0
            y = r * math.sin(angle) + 16.0
            z = i / 64.0
            pos3.append([x, y, z])
        pos3 = np.array(pos3)
        colors = np.array([[200, 0, 100, 200]])
        rerun.log_points(f"point3d", pos3, colors)

    if args.connect:
        time.sleep(1.0)  # HACK: give rerun time to send it all
    else:
        rerun.show()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Logs rich data using the Rerurn SDK.')
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    args = parser.parse_args()

    log(args)
