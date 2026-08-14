---
title: Working with LeRobot datasets
order: 750
description: Open and inspect LeRobot datasets in Rerun
---

The Rerun Viewer has built-in support for opening [LeRobot](https://huggingface.co/docs/lerobot/index) datasets, the directory-based format used for robot-learning datasets.
Both the v2.1 and v3.0 dataset layouts are supported.

## Quick start

### Loading a LeRobot dataset

A LeRobot dataset is a directory (metadata, parquet, and video files), so point Rerun at the dataset directory:

```bash
# View a LeRobot dataset in the Rerun Viewer
rerun path/to/lerobot_dataset
```

You can also drag and drop the dataset directory into the Rerun Viewer, or load it using the SDK:

snippet: howto/load_lerobot

## Data model

Each episode in the dataset is loaded as its own [recording](../../concepts/logging-and-ingestion/recordings.md).
Within an episode, the dataset's features are mapped onto Rerun [entities](../../concepts/logging-and-ingestion/entity-component.md): scalar observations and actions, camera images and videos, and the task descriptions are each logged under their own entity, indexed on a shared [timeline](../../concepts/logging-and-ingestion/timelines.md).

## Related

To go the other way — querying recordings and exporting them as a LeRobot dataset — see [Export recordings to LeRobot datasets](../train/lerobot_export.md).
