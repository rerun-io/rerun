import cv2
import math
import numpy as np
import time

import rerun_sdk as rerun

# Open up the viewer from within the Python process?
buffer = True

if buffer:
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

if buffer:
    rerun.show_and_quit()
else:
    time.sleep(1.0)  # HACK: give rerun time to send it all
