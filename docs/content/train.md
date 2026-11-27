---
title: Train
order: 8
description: Feed catalog recordings into training pipelines via export or PyTorch
---

A Rerun [catalog](catalog/catalog-object-model.md) can feed training pipelines two ways: export recordings to a standard format, or stream them directly into a PyTorch `DataLoader`.

The examples in this section use the local open-source server.
They also run against [Rerun Hub](catalog/hub.md), our commercial managed offering for production deployments, which offers a higher-performance backend for large-scale training. <!-- TODO(RR-6462): update this Rerun Hub description to the new branding -->
See [Run the examples on Hub](catalog/hub.md#run-the-examples-on-hub).

## Export to a training format

The catalog exposes recordings as queryable DataFrames via [DataFusion](https://datafusion.apache.org/python/).
Multi-rate sensor streams can be time-aligned and columns of interest extracted, with the result written to whatever format a training pipeline expects.

See [Export recordings to LeRobot datasets](train/lerobot_export.md) for a worked example.

## Train directly from the catalog

The experimental [`rerun.experimental.dataloader`](https://ref.rerun.io/docs/python/stable/experimental_dataloader/) module wraps a catalog as iterable or map-style PyTorch datasets, with no intermediate export step.

A sample is defined by a data source (a catalog dataset), an index timeline, and a set of fields, each decoded into a tensor.
Windows over the index express action chunks and observation history.
See [Train PyTorch models with Rerun](train/dataloader.md) for usage.
