import cv2
import time

import rerun_sdk


print(rerun_sdk.info())

rerun_sdk.log_point("point", 42, 1337)

img = cv2.imread('crates/re_viewer/data/logo_dark_mode.png',
                 cv2.IMREAD_UNCHANGED)
rerun_sdk.log_image("logo", img)


time.sleep(1.0)  # HACK: give rerun time to send it all
