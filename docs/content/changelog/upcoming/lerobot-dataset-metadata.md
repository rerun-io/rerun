---
title: LeRobot dataset metadata
hidden: true
type: feature
---

### LeRobot dataset metadata

`LeRobotReader.metadata()` returns the dataset-wide metadata of a LeRobot dataset as one static chunk at `/__lerobot` (configurable with `entity_path_prefix`), holding the text of `meta/info.json` and, when present, `meta/stats.json`.
The metadata is the same for every episode, so write it to its own `.rrd` and register it once with `DatasetEntry.register_asset()` instead of copying it into each episode.
The dataloader example now reads feature shapes and normalization statistics from this asset.

[Train a policy on a Rerun dataset](../getting-started/train.md)
[Dataloader example](https://github.com/rerun-io/rerun/tree/main/examples/python/dataloader)
