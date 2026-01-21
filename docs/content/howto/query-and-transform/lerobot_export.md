---
title: Export robot recordings to LeRobot datasets
order: 90
---

Robot data logged with Rerun can be converted into training-ready datasets for imitation learning.
This guide demonstrates how to use the OSS Rerun server to query RRD recordings, align multi-rate sensor data to a common timeline, and export the result as a [LeRobot v3 dataset](https://github.com/huggingface/lerobot).

The workflow transforms Rerun's flexible, sparse time series data into LeRobot's standardized tabular format, handling:

- **Time alignment**: Synchronizing sensors that fire at different rates
- **Data resampling**: Creating fixed-framerate outputs from variable-rate inputs
- **Video encoding**: Extracting and encoding video streams efficiently
- **Schema mapping**: Converting Rerun's component model to LeRobot's feature definitions

## Prerequisites

This example requires the `rerun_export` package from the Rerun repository:

```bash
pip install -e examples/python/rerun_export
```

This will install the necessary dependencies including LeRobot, DataFusion, and PyArrow.

## Setup

Start a local server and load your RRD recordings. Each recording becomes a segment in the dataset.

snippet: howto/lerobot_export[setup]

## Filter data for training

Robot recordings often contain more data than needed for training. Filter the dataset to include only the relevant entity paths and components.

snippet: howto/lerobot_export[filter_data]

The `reader()` method creates a dataframe view that will be used for the export. Specifying `index="real_time"` means all data will be aligned to the real-time timeline.

## Configure the export

Define how to map Rerun's data to LeRobot's standardized format. This requires specifying:

- Which components contain actions and observations
- Video streams to include
- Target framerate for the dataset
- Timeline to use for alignment

snippet: howto/lerobot_export[configure_export]

### Understanding column specifications

The `rerun_export` tool uses Rerun's fully qualified column names to identify exactly which data to extract. In Rerun's data model, columns are named in the format:

```
/entity/path:ComponentType:field_name
```

For example:

- `/action/joint_positions:Scalars:scalars` -> Extract the `scalars` field from the [Scalars](../../reference/types/archetypes/scalars.md) archetype at `/action/joint_positions`
- `/language_instruction:TextDocument:text` -> Extract the `text` field from a [TextDocument](../../reference/types/archetypes/text_document.md) archetype at `/language_instruction`

You can discover available columns using the dataset schema:

```python
schema = dataset.schema()
for col in schema.component_columns():
    print(col)
```

### Video stream configuration

Each video stream needs:

- **key**: A short identifier used in the LeRobot dataset (e.g., `"front"`, `"wrist"`)
- **path**: Entity path where the `VideoStream` component is logged
- **video_format**: Encoding format (`"h264"`, `"h265"`, etc.)

## Infer feature schema

LeRobot uses a schema called "features" to describe dataset structure. The `infer_features` function automatically creates this schema by inspecting your data.

snippet: howto/lerobot_export[infer_features]

Feature inference examines the underlying data to determine:

- Data types (float, int, image, video, text)
- Array shapes (scalar vs. vector vs. matrix)
- Video dimensions (by decoding the first frame)

## Create the LeRobot dataset

Initialize the output dataset structure on disk. This creates the directory hierarchy and prepares video encoding pipelines.

snippet: howto/lerobot_export[create_dataset]

The `LeRobotDataset.create()` method:

- Creates `data/`, `videos/`, and `meta/` directories
- Initializes Parquet files for time series data
- Sets up video encoders for each camera stream
- Writes dataset metadata (fps, features, repo ID)

## Export the episode

Convert the filtered, aligned data into a LeRobot episode. This is the core transformation step.

snippet: howto/lerobot_export[export_episode]

The `convert_dataframe_to_episode` function resamples the dataframe to the target framerate and writes it to LeRobot's format. The tabular data (actions, states, task descriptions) is written to Parquet files, while video streams are efficiently remuxed without re-encoding.

The `finalize()` call completes the dataset by writing metadata and closing all files.

## Multi-episode export

To export multiple recordings as separate episodes, iterate over segment IDs:

```python
# Get all segment IDs in the dataset
segment_ids = dataset.segment_ids()

for segment_id in segment_ids:
    print(f"Processing episode: {segment_id}")

    # Filter to this segment
    training_data = (
        dataset.filter_segments(segment_id)
        .filter_contents(["/action/**", "/observation/**", "/camera/**"])
        .reader(index="real_time")
    )

    # Update config for this segment
    config = LeRobotConversionConfig(
        fps=15,
        index_column="real_time",
        action="/action/joint_positions:Scalars:scalars",
        state="/observation/joint_positions:Scalars:scalars",
        videos=videos,
        dataset=dataset,
        segment_id=segment_id,
    )

    # Convert the episode
    convert_dataframe_to_episode(
        df=training_data,
        config=config,
        lerobot_dataset=lerobot_dataset,
        segment_id=segment_id,
        features=features,
    )

# Finalize once after all episodes
lerobot_dataset.finalize()
```

## Time alignment and resampling

By default, the export uses the framerate specified in the config to create evenly spaced samples. Under the hood, this uses the `using_index_values` parameter with `fill_latest_at=True`:

```python
# Create a time grid at the target framerate
desired_times = make_time_grid(min_time, max_time, fps=15)

# Query data at those exact times, forward-filling gaps
df = dataset.reader(
    index="real_time",
    using_index_values=desired_times,
    fill_latest_at=True
)
```

This ensures:

- All sensors are aligned to the same timestamps
- Missing values are forward-filled from the most recent observation
- The output has a consistent framerate

For more details on time alignment, see [Time-align data](time_alignment.md).

## Handling missing data

Real-world robot data often has:

- **Startup delays**: Some sensors start logging later than others
- **Dropouts**: Intermittent sensor failures or network issues
- **Different rates**: Joint encoders at 100Hz, cameras at 30Hz, etc.

The export process handles these cases:

```python
# Skip episodes where critical data is missing
if not has_required_data(training_data):
    print(f"Skipping {segment_id}: missing required components")
    continue

# Filter out rows where action or state is null
# (can happen if one starts before the other)
df = df.filter(
    col("/action/joint_positions:Scalars:scalars").is_not_null(),
    col("/observation/joint_positions:Scalars:scalars").is_not_null(),
)
```

## Using the exported dataset

The exported LeRobot dataset can be used directly with LeRobot's training scripts:

```python
from lerobot.datasets.lerobot_dataset import LeRobotDataset

# Load the exported dataset
dataset = LeRobotDataset(
    repo_id="droid/gripper-closing",
    root="./lerobot_dataset",
)

# Access episode data
print(f"Total episodes: {dataset.num_episodes}")
print(f"Total frames: {len(dataset)}")

# Get a sample
sample = dataset[0]
print(sample.keys())
```

Or push it to the Hugging Face Hub for sharing:

```python
dataset.push_to_hub(repo_id="your-username/your-dataset-name")
```

## Example: CLI for batch conversion

The `rerun_export` package includes a CLI that implements this workflow for batch processing:

```bash
rerun_export \
  --rrd-dir ./robot_recordings \
  --output ./lerobot_dataset \
  --dataset-name robot_demos \
  --fps 15 \
  --action /action/joint_positions:Scalars:scalars \
  --state /observation/joint_positions:Scalars:scalars \
  --task /language_instruction:TextDocument:text \
  --video front:/camera/front \
  --video wrist:/camera/wrist
```

See the [rerun_export example](https://github.com/rerun-io/rerun/tree/main/examples/python/rerun_export) for the complete implementation.

## Next steps

- [Time-align data](time_alignment.md): Learn more about time alignment strategies
- [Common dataframe operations](dataframe_operations.md): Advanced querying and filtering
- [Query data out of Rerun](get-data-out.md): Fundamentals of the dataframe API
