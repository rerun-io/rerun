#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
import os
from pathlib import Path
from typing import Final

import cv2
import numpy as np
from PIL import Image
import rerun_sdk as rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
CACHE_DIR: Final = Path(os.path.dirname(__file__)) / "cache"

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import pipeline

object_detector = pipeline("object-detection")
label_map = object_detector.model.config.id2label

image_path = str(DATASET_DIR / "street_mc.jpeg")
image = cv2.cvtColor(cv2.cv2.imread(image_path), cv2.COLOR_BGR2RGB)
rerun.log_image("image", image)

pil_im = Image.fromarray(image)


detections = object_detector(pil_im)

rects = [list(det["box"].values()) for det in detections]
rects = np.array(rects)
labels = [det["label"] for det in detections]
rerun.log_rects("image/detections", rects, rect_format=rerun.RectFormat.XYXY, labels=labels)

rerun.show()
