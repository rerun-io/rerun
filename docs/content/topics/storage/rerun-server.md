---
title: Rerun Server
order: 60
---

Understanding tables and datasets on Rerun Server.

> **Placeholder**: This section will be expanded with detailed documentation about how data is organized on Rerun Server.

## Overview

Rerun Server provides a centralized way to store and manage your data. Data is organized into:

- **Tables**: Structured collections of data with defined schemas
- **Datasets**: Logical groupings of related recordings and tables

## Tables

Tables in Rerun Server store structured data that can be queried efficiently. They support:

- Time-based indexing for efficient temporal queries
- Flexible schemas that accommodate different data types
- Integration with dataframe APIs for analysis

## Datasets

Datasets provide a way to organize related data together. A dataset might contain:

- Multiple recordings from the same experiment
- Different sensor modalities collected together
- Training and evaluation data splits

## Related topics

- [RRD Format](rrd-format.md): Rerun's native file format
- [Apps and Recordings](apps-and-recordings.md): How recordings work
- [Dataframes](../query-semantics/dataframes.md): Querying data as dataframes
