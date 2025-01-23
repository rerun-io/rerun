<!--[metadata]
title = "Drone LiDAR"
tags = ["3D", "drone", "Lidar"]
description = "Display drone-based LiDAR data"
-->

<picture>
  <img src="https://static.rerun.io/drone_lidar/95c49d78abc01513d344c06e2d9a0c8b84376a0d/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/drone_lidar/95c49d78abc01513d344c06e2d9a0c8b84376a0d/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/drone_lidar/95c49d78abc01513d344c06e2d9a0c8b84376a0d/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/drone_lidar/95c49d78abc01513d344c06e2d9a0c8b84376a0d/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/drone_lidar/95c49d78abc01513d344c06e2d9a0c8b84376a0d/1200w.png">
</picture>


## Background

This example displays drone-based indoor LiDAR data loaded from a [`.las`](https://en.wikipedia.org/wiki/LAS_file_format) file. This dataset contains 18.7M points, acquired at 4013 distinct time points (~4650 points per time point). The point data is loaded using the [laspy](https://laspy.readthedocs.io/en/latest/) Python package, and then sent in one go to the viewer thanks to the [`rr.send_columns_v2()`](https://ref.rerun.io/docs/python/0.18.2/common/columnar_api/#rerun.send_columns) API and its `.partition()` helper. Together, these APIs enable associating subgroups of points with each of their corresponding, non-repeating timestamps.


[Flyability](https://www.flyability.com) kindly provided the data for this example.


## Running

Install the example package:
```bash
pip install -e examples/python/drone_lidar
```

To experiment with the provided example, simply execute the main Python script:
```bash
python -m drone_lidar
```

If you wish to customize it, explore additional features, or save it, use the CLI with the `--help` option for guidance:

```bash
python -m drone_lidar --help
```
