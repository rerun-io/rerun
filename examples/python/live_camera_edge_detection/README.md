---
title: Live Camera Edge Detection
python: https://github.com/rerun-io/rerun/blob/latest/examples/python/live_camera_edge_detection/main.py
tags: 2D, canny, live, opencv
---

Very simple example of capturing from a live camera.

Runs the opencv canny edge detector on the image stream.

NOTE: this example currently runs forever and will eventually exhaust your
system memory. It is advised you run an independent rerun viewer with a memory
limit:
```
rerun --memory-limit 4GB
```

And then connect using:
```
python examples/python/live_camera_edge_detection/main.py --connect
```
