---
title: Export recordings to LeRobot datasets
order: 90
---

Convert Rerun data into the training dataset version of your choice.
This guide demonstrates how to use the OSS Rerun server to query recordings, align multi-rate sensor data to a common timeline, and export the result as a [LeRobot](https://github.com/huggingface/lerobot) dataset.

## Prerequisites

This example requires the `rerun_export` package from the Rerun repository:

```bash
pip install -e examples/python/rerun_export
```

This will install the necessary dependencies including LeRobot, DataFusion, and PyArrow.

## Time alignment and resampling

By default, the export uses the frame rate specified in the config to create evenly spaced samples (a LeRobot requirement).

For more details on time alignment, see [Time-align data](time_alignment.md).

## Setup

Start a local server and load your recordings.
Each recording becomes a segment in the dataset, and each unique segment id becomes one LeRobot episode.

snippet: howto/lerobot_export[setup]

See [Catalog object model](../../concepts/query-and-transform/catalog-object-model.md) for how recordings are represented on the Data Platform.

### Filter data for training

Robot recordings often contain more data than needed for training.
Filter the dataset to include only the relevant entity paths and components that will map to LeRobot’s standardized format.

For example, you might include joint position commands as actions, joint states and end-effector pose as observations, RGB camera streams as video inputs, and a language instruction as the task description.
Other signals such as debug visualizations, intermediate computations, or unused sensors can be excluded.

snippet: howto/lerobot_export[filter_data]

### Configure the export

Define how to map the data to LeRobot's standardized format. This requires specifying:

- Which components contain actions and observations
- Video streams to include
- Target frame rate for the dataset
- Timeline to use for alignment

snippet: howto/lerobot_export[configure_export]

### Infer feature schema

LeRobot uses a schema called "features" to describe dataset structure. The `infer_features` function automatically creates this schema by inspecting your data.

snippet: howto/lerobot_export[infer_features]

Feature inference examines the underlying data to determine:

- Data types (float, int, image, video, text)
- Array shapes (scalar, vector, matrix)
- Video dimensions (by decoding the first frame)

## Create the LeRobot dataset

Create the LeRobot dataset instance, using the LeRobot dataset API:

snippet: howto/lerobot_export[create_dataset]

Note, `root` is where the dataset files will be written, and LeRobot requires this to be an empty or non-existing directory.

### Export the episode

Convert the filtered data into a LeRobot episode. This is the core transformation step.

snippet: howto/lerobot_export[export_episode]

The `convert_dataframe_to_episode` function performs time alignment and resamples the dataframe to the target frame rate. It generates a sequence of evenly spaced timestamps at the target frame rate and treats these as the canonical timesteps for the episode.
For each timestep, it queries the most recent available value of every selected component using Rerun’s [`latest-at`](../../concepts/logging-and-ingestion/latest-at.md) semantics. If a stream has no sample exactly at that time, its last observed value is forward-filled.

The `finalize()` call completes the dataset by writing metadata and closing all files.

### Multi-episode export

To export multiple recordings as separate episodes, iterate over segment IDs:

snippet: howto/lerobot_export[multi_episode_export]

## Using the exported dataset

The exported LeRobot dataset can be used directly with LeRobot's training scripts:

snippet: howto/lerobot_export[use_dataset]

Or push it to the Hugging Face Hub for sharing:

```python
dataset.push_to_hub(repo_id="your-username/your-dataset-name")
```

## Command-line interface

The `rerun_export` package includes a CLI that implements this workflow for batch processing:

```bash
rerun_export \
  --rrd-dir ./tests/assets/rrd/sample_5 \
  --output ./lerobot_dataset \
  --dataset-name rerun-example-droid \
  --fps 15 \
  --action /action/joint_positions:Scalars:scalars \
  --state /observation/joint_positions:Scalars:scalars \
  --task /language_instruction:TextDocument:text \
  --video ext1:/camera/ext1 \
  --video ext2:/camera/ext2 \
  --video wrist:/camera/wrist
```

See the [rerun_export example](../../../../examples/python/rerun_export) for the complete implementation.
