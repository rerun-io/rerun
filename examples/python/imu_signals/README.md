<!--[metadata]
title = "IMU signals"
tags = ["Plots"]
description = "Log multi dimensional signals under a single entity."
thumbnail = "https://static.rerun.io/imu_signals/64f773d238a0456a0f233abeea7e521cfb871b67/480w.jpg"
thumbnail_dimensions = [480, 480]
channel = "main"
include_in_manifest = true
build_args = ["--seconds=10"]
-->

This example demonstrates how to log multi dimensional signals with the Rerun SDK, using the [TUM VI Benchmark](https://cvg.cit.tum.de/data/datasets/visual-inertial-dataset).

<picture>
  <img src="https://static.rerun.io/imu_signals/1184ab6e2df3275b8b7a574d7f0e42b1aed8343a/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/imu_signals/1184ab6e2df3275b8b7a574d7f0e42b1aed8343a/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/imu_signals/1184ab6e2df3275b8b7a574d7f0e42b1aed8343a/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/imu_signals/1184ab6e2df3275b8b7a574d7f0e42b1aed8343a/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/imu_signals/1184ab6e2df3275b8b7a574d7f0e42b1aed8343a/1200w.png">
</picture>

## Background

This example shows how to log multi-dimensional signals efficiently using the [`rr.send_columns()`](https://ref.rerun.io/docs/python/0.22.1/common/columnar_api/#rerun.send_columns) API.

The API automatically selects the right partition sizes, making it simple to log scalar signals like this:

```py
# Load IMU data from CSV into a dataframe
imu_data = pd.read_csv(
    cwd / DATASET_NAME / "dso/imu.txt",
    sep=" ",
    header=0,
    names=["timestamp", "gyro.x", "gyro.y", "gyro.z", "accel.x", "accel.y", "accel.z"],
    comment="#",
)

times = rr.TimeColumn("timestamp", timestamp=imu_data["timestamp"])

# Extract gyroscope data (x, y, z axes) and log it to a single entity.
gyro = imu_data[["gyro.x", "gyro.y", "gyro.z"]]
rr.send_columns("/gyroscope", indexes=[times], columns=rr.Scalars.columns(scalars=gyro))

# Extract accelerometer data (x, y, z axes) and log it to a single entity.
accel = imu_data[["accel.x", "accel.y", "accel.z"]]
rr.send_columns("/accelerometer", indexes=[times], columns=rr.Scalars.columns(scalars=accel))
```

## Running

Install the example package:

```bash
pip install -e examples/python/imu_signals
```

To experiment with the provided example, simply execute the main Python script:

```bash
python -m imu_signals
```

## Attribution

This example uses a scene from the **TUM VI Benchmark dataset**, originally provided by [Technical University of Munich (TUM)](https://cvg.cit.tum.de/data/datasets/visual-inertial-dataset).
The dataset is licensed under **Creative Commons Attribution 4.0 (CC BY 4.0)**.

- Original dataset: [TUM VI Benchmark](https://cvg.cit.tum.de/data/datasets/visual-inertial-dataset)
- License details: [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)
