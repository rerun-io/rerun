---
title: "Measure dataloader pipeline performance"
hidden: true
type: feature
---

### Measure dataloader pipeline performance

`RerunIterableDataset` now collects cumulative fetch and decode metrics automatically across PyTorch `DataLoader` workers.
Read `dataset.metrics.summary()` to get a flat dictionary of fetch versus decode time, direct HTTP range merging, GOP decode amplification, and skipped samples.
An optional prefix makes the dictionary directly usable with experiment trackers such as MLflow and Weights & Biases.
Reports are cumulative aggregates of completed worker measurements.

Docs: ../howto/train/dataloader.md
