---
title: Creating sub-datasets
order: 115
---

When experimenting with new features it's often practical to work with a subset of data without modifying the original.
A sub-dataset references the same underlying RRD files so no data is copied.

The dependencies in this example are contained in `rerun-sdk[all]`.

## Setup

Simplified setup to launch the local server for demonstration.
In practice you'll connect to your cloud instance.

snippet: howto/sub_dataset[setup]

## Helper function

Query the source dataset's [manifest](../../concepts/query-and-transform/catalog-object-model.md) for storage URLs per (segment, layer) pair and re-register them into a new dataset.

snippet: howto/sub_dataset[create_sub_dataset]

## Selecting segments

Select segments by any criteria — a hardcoded list, a slice, or a filtered query based on segment properties or metadata joins.

snippet: howto/sub_dataset[select_segments]

## Creating the sub-dataset

snippet: howto/sub_dataset[create]

## Verifying the result

snippet: howto/sub_dataset[verify]

## Cleanup

Delete the sub-dataset when it is no longer needed.
This only removes the dataset entry from the catalog. The underlying RRD storage is not affected.

snippet: howto/sub_dataset[cleanup]
