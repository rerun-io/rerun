#!/usr/bin/env python3
"""Example applying simple object detection and tracking on a video."""
import os
from pathlib import Path
from typing import Final

import cv2 as cv
import numpy as np
from PIL import Image
import torch
import rerun_sdk as rerun

DATASET_DIR: Final = Path(os.path.dirname(__file__)) / "dataset"
CACHE_DIR: Final = Path(os.path.dirname(__file__)) / "cache"

os.environ["TRANSFORMERS_CACHE"] = str(CACHE_DIR.absolute())
from transformers import DetrFeatureExtractor, DetrForObjectDetection

rerun.connect()

feature_extractor = DetrFeatureExtractor.from_pretrained("facebook/detr-resnet-50")
model = DetrForObjectDetection.from_pretrained("facebook/detr-resnet-50")

car_id = model.config.label2id["car"]

video_path = str(DATASET_DIR / "a-car-drifting-in-asphalt-road-4569076-short.mp4")

cap = cv.VideoCapture(video_path)
frame_idx = 0

tracker = None
detection_is_visible = False
while cap.isOpened():
    ret, bgr = cap.read()
    rerun.set_time_sequence("frame", frame_idx)

    if not ret:
        print("End of video")
        break

    rgb = cv.cvtColor(bgr, cv.COLOR_BGR2RGB)
    rerun.log_image("video", rgb)

    if tracker is None or frame_idx % 40 == 0:
        pil_im = Image.fromarray(rgb)

        inputs = feature_extractor(images=pil_im, return_tensors="pt")
        outputs = model(**inputs)

        target_sizes = torch.tensor([pil_im.size[::-1]])
        results = feature_extractor.post_process_object_detection(outputs, target_sizes=target_sizes)[0]

        labels = results["labels"].detach().cpu().numpy()
        boxes = results["boxes"].detach().cpu().numpy()

        car_boxes = boxes[labels == car_id, :]
        num_cars = car_boxes.shape[0]
        labels = ["car" for _ in range(num_cars)]
        # TODO(Niko): Remove workaround for PRO-213 and PRO-214
        rerun.log_rects("image/detections", car_boxes, rect_format=rerun.RectFormat.XYXY, labels=labels)

        if num_cars > 0:
            top_car_bbox = car_boxes[0, :]
            # TODO(Niko): Remove workaround for PRO-213 and PRO-214
            rerun.log_rect("image/detections", top_car_bbox, rect_format=rerun.RectFormat.XYXY, label="car")
            rerun.set_visible("image/detections", True)
            detection_is_visible = True
            x_min, y_min, x_max, y_max = top_car_bbox.tolist()
            bbox_xywh = [x_min, y_min, x_max - x_min, y_max - y_min]
            bbox_xywh = [int(val) for val in bbox_xywh]
            tracker = cv.TrackerCSRT_create()
            tracker.init(bgr, bbox_xywh)

            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="tracked_car")
    else:
        if detection_is_visible:
            rerun.set_visible("image/detections", False)
            detection_is_visible = False

        success, bbox_xywh = tracker.update(bgr)

        if success:
            rerun.log_rect("image/tracked", bbox_xywh, rect_format=rerun.RectFormat.XYWH, label="tracked_car")
        else:
            tracking = False
            rerun.set_visible("image/tracked", False)

    frame_idx += 1
