<!--[metadata]
title = "LeRobot dataset from RRD"
tags = ["Robotics", "MCAP", "LeRobot", "Dataset", "Server"]
thumbnail = "https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/480w.png"
thumbnail_dimensions = [480, 384]
-->

<picture>
  <img src="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun_export/f3b727db8bbe3ecf6894707ac7770d3d8fc8bf1f/1200w.png">
</picture>

Convert robot recordings into training-ready datasets by using the OSS Rerun server to query and transform RRD files into LeRobot v3 format.

## Background

This example demonstrates how to use the Rerun OSS server API to process robot recordings and prepare them for imitation learning.
The workflow uses the server to load RRD files, query robot data (actions, observations, videos), align time series to a target framerate, and write the result as a LeRobot v3 dataset compatible with robotics model training pipelines.

[LeRobot](https://github.com/huggingface/lerobot) is a project by Hugging Face that provides models, datasets, and tools for real-world robotics in PyTorch. This example shows how Rerun recordings can be converted into LeRobot's standardized dataset format.

## Conversion workflow

The converter loads RRD files into the OSS server, infers data types from the recordings, resamples all data to a target framerate, and writes the result as a LeRobot v3 dataset. Video streams are efficiently remuxed without re-encoding.

## Run the code

### Installation

To install the example and its dependencies:

```bash
pip install -e examples/python/rerun_export
```

### Basic usage

The example provides a CLI that converts a directory of RRD recordings into a LeRobot v3 dataset:

```bash
rerun_export \
  --rrd-dir /path/to/recordings \
  --output /path/to/output/dataset \
  --dataset-name my_robot_dataset \
  --fps 10 \
  --index real_time \
  --action /action:Scalars:scalars \
  --state /observation/joint_positions:Scalars:scalars \
  --task /language_instruction:TextDocument:text \
  --video front:/camera/front:VideoStream:sample
```

### Video specification format

Videos are specified as `key:path`:

- `key`: Camera identifier (e.g., `front`, `wrist`)
- `path`: Entity path to the video stream (e.g., `/camera/front`)

The converter expects [VideoStream](https://www.rerun.io/docs/reference/types/archetypes/video_stream), components at the specified paths.

## Example workflow

Here's a complete example converting simulated robot teleop data:

```bash
# Convert RRD recordings to LeRobot dataset
rerun_export \
  --rrd-dir ./robot_recordings \
  --output ./lerobot_dataset \
  --dataset-name robot_demos \
  --fps 15 \
  --action /robot/action:Scalars:scalars \
  --state /robot/state:Scalars:scalars \
  --task /task:TextDocument:text \
  --video front:/camera/front:VideoStream:sample \
  --video wrist:/camera/wrist:VideoStream:sample \
  --action-names "joint_0,joint_1,joint_2,gripper" \
  --state-names "joint_0,joint_1,joint_2,gripper"

# The resulting dataset can be used with LeRobot training scripts
```

The output directory will contain:

- `data/`: Parquet files with aligned time series data
- `videos/`: Encoded video files (if using `--use-videos`)
- `meta/`: Dataset metadata and episode information
