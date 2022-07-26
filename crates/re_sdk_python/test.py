import cv2
import math
import time

import rerun_sdk


print(rerun_sdk.info())

img = cv2.imread('crates/re_viewer/data/logo_dark_mode.png',
                 cv2.IMREAD_UNCHANGED)
rerun_sdk.log_image("logo", img)

for i in range(64):
    angle = 6.28 * i / 64
    r = 20.0
    x = r * math.cos(angle) + 18.0
    y = r * math.sin(angle) + 16.0
    rerun_sdk.log_point(f"point_{i}", x, y)


time.sleep(1.0)  # HACK: give rerun time to send it all
