---
title: Docs Guide
order: 400
---

This page provides an overview of how the Rerun documentation is organized to help you find what you need.

## Overview

High-level introduction to Rerun:

- **[What is Rerun?](what-is-rerun.md)** - Learn about Rerun's data platform for Physical AI
- **[Installing the Viewer](installing-viewer.md)** - Get Rerun installed on your system
- **[Troubleshooting](troubleshooting.md)** - Solutions to common issues
- **[Application Model](app-model.md)** - How Rerun applications are structured

## Getting started

Step-by-step guides to get up and running quickly:

- **[Send Data](../getting-started/data-in.md)** - Learn how to log data to Rerun from your code
- **[See Data](../getting-started/configure-the-viewer.md)** - Customize the visualization to your needs
- **[Query Data](../getting-started/data-out.md)** - Query and export data from Rerun recordings
- **[Build Data Pipelines](../getting-started/build-data-pipeline.md)** - Production data infrastructure

## Concepts

Understanding the foundational concepts behind Rerun:

### Data model
- **[Entity Component System](../concepts/data-model/entity-component.md)** - Rerun's data model
- **[Entity Paths](../concepts/data-model/entity-path.md)** - Organizing your data hierarchically
- **[Transforms](../concepts/data-model/transforms.md)** - Working with coordinate systems
- **[Timelines](../concepts/data-model/timelines.md)** - Managing temporal data
- **[Batches](../concepts/data-model/batches.md)** - Efficiently logging collections of data
- **[Static Data](../concepts/data-model/static.md)** - Data that exists across all timelines
- **[Chunks](../concepts/data-model/chunks.md)** - Internal storage mechanism (advanced)
- **[Video](../concepts/data-model/video.md)** - Video data handling

### Building visualization
- **[Blueprints](../concepts/building-visualization/blueprints.md)** - Configuring visualization layouts
- **[Visualizers and Overrides](../concepts/building-visualization/visualizers-and-overrides.md)** - Customizing rendering
- **[Annotation Context](../concepts/building-visualization/annotation-context.md)** - Shared styling and labels

### Storage
- **[Apps and Recordings](../concepts/storage/apps-and-recordings.md)** - Managing application and recording IDs
- **[Sinks](../concepts/storage/sinks.md)** - Where data can be sent
- **[Data-loaders](../concepts/storage/data-loaders.md)** - Loading data from external sources
- **[MCAP](../concepts/storage/mcap.md)** - MCAP file format support
- **[RRD Format](../concepts/storage/rrd-format.md)** - Rerun's native data format

### Query semantics
- **[Latest-at Semantics](../concepts/query-semantics/latest-at.md)** - How Rerun resolves data queries
- **[Entity Queries](../concepts/query-semantics/entity-queries.md)** - Selecting entities for views
- **[Dataframes](../concepts/query-semantics/dataframes.md)** - Working with data as dataframes

## Cookbook

Practical guides for specific tasks and advanced features:

### Logging data
- **[Logging](../cookbook/logging.md)** - Advanced logging techniques
- **[Send Columns](../cookbook/send_columns.md)** - Efficiently log columnar data
- **[Using Native Loggers](../cookbook/using-native-loggers.md)** - Integrate with existing logging systems
- **[Short-lived Entities](../cookbook/short-lived-entities.md)** - Handling temporary data

### Visualization
- **[Visualization](../cookbook/visualization.md)** - Advanced visualization techniques
- **[Configure Viewer Through Code](../cookbook/configure-viewer-through-code.md)** - Programmatic viewer configuration
- **[Fixed Window Plots](../cookbook/fixed-window-plot.md)** - Creating time-windowed plots

### Data management
- **[DataFrame API](../cookbook/dataframe-api.md)** - Query recordings programmatically
- **[Get Data Out](../cookbook/get-data-out.md)** - Export data from Rerun
- **[MCAP Integration](../cookbook/mcap.md)** - Working with MCAP files
- **[Shared Recordings](../cookbook/shared-recordings.md)** - Collaborate with recordings

### Integration & deployment
- **[Integrations](../cookbook/integrations.md)** - Integrate Rerun with other tools
- **[Embed Rerun Viewer](../cookbook/embed-rerun-viewer.md)** - Embed the viewer in your application
- **[Jupyter Notebooks](../cookbook/notebook.md)** - Use Rerun in notebooks
- **[Callbacks](../cookbook/callbacks.md)** - Respond to viewer events

### Performance & optimization
- **[Limit RAM Usage](../cookbook/limit-ram.md)** - Control memory consumption
- **[Optimize Chunks](../cookbook/optimize-chunks.md)** - Fine-tune data storage

### Extending Rerun
- **[Extend](../cookbook/extend.md)** - Add custom types and visualizations

### Examples
- **[ROS2 Nav Turtlebot](../cookbook/ros2-nav-turtlebot.md)** - Complete robotics example

## Reference

Detailed API documentation and technical specifications:

- **[Reference Documentation](../reference.md)** - Complete API reference for all supported languages
  - Types (Archetypes, Components, Datatypes)
  - SDKs (Python, Rust, C++)
  - Viewer and CLI commands
  - Migration guides

## Development

Contributing to Rerun:

- **[Developing Rerun](../development.md)** - How to contribute to the Rerun project

## Community

- [Rerun Discord](https://discord.gg/PXtCgFBSmH) - Join the community
- [GitHub Repository](https://github.com/rerun-io/rerun) - Source code and issue tracking
- [Examples Gallery](https://rerun.io/examples) - See Rerun in action
