<!--[metadata]
title = "LeRobot dataset curation using OSS Rerun server"
tags = ["Robotics", "MCAP", "LeRobot", "Dataset", "Server"]
channel = "main"
thumbnail = "https://static.rerun.io/oss-server-lerobot/placeholder/480w.png"
thumbnail_dimensions = [480, 480]
-->


# TODO(gijsd): Claude wrote this readme, do another pass
Also make sure to add the proper thumbnail images once available!

<!-- <picture>
  <img src="https://static.rerun.io/oss-server-lerobot/placeholder/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/oss-server-lerobot/placeholder/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/oss-server-lerobot/placeholder/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/oss-server-lerobot/placeholder/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/oss-server-lerobot/placeholder/1200w.png">
</picture> -->

An example of how to use the OSS Rerun server to inspect and prepare MCAP recordings for training by constructing a LeRobot v3 dataset.

## Logging and visualizing with Rerun

This example demonstrates how to:

1. Start a local Rerun OSS server and load MCAP recordings
2. Inspect and query robot telemetry data through the server API
3. Prepare and structure the data into LeRobot v3 dataset format
4. Validate the dataset is ready for robotics model training

The key steps are:

## Conversion CLI

This example ships a CLI that reads a directory of `.rrd` recordings via the OSS server API, aligns them
at a target FPS, and writes a LeRobot v3 dataset to disk.

```bash
oss_server_lerobot \
  --rrd-dir /path/to/rrds \
  --output /tmp/lerobot_dataset \
  --dataset-name robot_runs \
  --fps 10 \
  --index real_time \
  --action-path /action \
  --state-path /observation/joint_positions \
  --task-path /language_instruction \
  --image front:/camera/front:raw
```

Notes:
- Use `--image` multiple times to add more cameras (format: `key:path:kind`, where `kind` is `raw`, `compressed`, or `video`).
- By default, `--action-path` and `--state-path` assume the standard `Scalars:scalars` archetype. To specify custom archetypes, include the full column path with colons (e.g., `--action-path /robot_right/joint_states:schemas.proto.JointState:joint_positions`).
- If your schema differs, override column names with `--action-column`, `--state-column`, or `--task-column`.
- For video streams, pass `--video-format` if the stream is not H.264.
