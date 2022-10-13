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
while cap.isOpened():
    ret, frame = cap.read()
    rerun.set_time_sequence("frame", frame_idx)

    if not ret:
        print("End of video")
        break

    rgb = cv.cvtColor(frame, cv.COLOR_BGR2RGB)
    rerun.log_image("video", rgb)

    pil_im = Image.fromarray(rgb)

    if frame_idx % 20 == 0:
        inputs = feature_extractor(images=pil_im, return_tensors="pt")
        outputs = model(**inputs)

        target_sizes = torch.tensor([pil_im.size[::-1]])
        results = feature_extractor.post_process_object_detection(outputs, target_sizes=target_sizes)[0]

        labels = results["labels"].detach().cpu().numpy()
        boxes = results["boxes"].detach().cpu().numpy()

        car_boxes = boxes[labels == car_id, :]
        num_cars = car_boxes.shape[0]
        labels = ["car" for _ in range(num_cars)]
        rerun.log_rects("image/detections", car_boxes, rect_format=rerun.RectFormat.XYXY, labels=labels)

    frame_idx += 1
