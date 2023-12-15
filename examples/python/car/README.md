---
title = "Car"
thumbnail = "https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/480w.png"
thumbnail_dimensions = [480, 235]
---

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/1200w.png">
  <img src="https://static.rerun.io/car/014857675dfed92c3f6a9492e3291c48a982ac83/full.png" alt="Car example screenshot">
</picture>


A very simple 2D car is drawn using OpenCV, and a depth image is simulated and logged as a point cloud.

```bash
pip install -r examples/python/car/requirements.txt
python examples/python/car/main.py
```