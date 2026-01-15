---
title: Docs Guide
order: 400
---

This page provides an overview of how the Rerun documentation is organized to help you find what you need.

## Overview

High-level introduction to Rerun:

- **[What is Rerun?](what-is-rerun.md)** - Learn about Rerun's data platform for Physical AI
- **[Installing the Viewer](installing-rerun/viewer.md)** - Get Rerun installed on your system

## Getting started

Step-by-step guides to get up and running quickly:

- **[Log and Ingest](../getting-started/data-in.md)** - Learn how to log data to Rerun from your code
- **[Visualize](../getting-started/configure-the-viewer.md)** - Customize the visualization to your needs
- **[Query and Transform](../getting-started/data-out.md)** - Query and export data from Rerun recordings
- **[Troubleshooting](../overview/installing-rerun/troubleshooting.md)** - Solutions to common issues

## Concepts

Understanding the foundational concepts behind Rerun:

- **[How Does Rerun Work](../concepts/how-does-rerun-work.md)** - How Rerun applications are structured
- **[Entity Component System](../concepts/logging-and-ingestion/entity-component.md)** - Rerun's data model
- **[Entity Paths](../concepts/logging-and-ingestion/entity-path.md)** - Organizing your data hierarchically
- **[Spaces and Transforms](../concepts/logging-and-ingestion/transforms.md)** - Working with coordinate systems
- **[Timelines](../concepts/logging-and-ingestion/timelines.md)** - Managing temporal data
- **[Blueprints](../concepts/visualization/blueprints.md)** - Configuring visualization layouts
- **[Batches](../concepts/logging-and-ingestion/batches.md)** - Efficiently logging collections of data
- **[Static Data](../concepts/logging-and-ingestion/static.md)** - Data that exists across all timelines
- **[Query Semantics](../concepts/logging-and-ingestion/latest-at.md)** - How Rerun resolves data queries
- **[Annotation Context](../concepts/visualization/annotation-context.md)** - Shared styling and labels
- **[Apps and Recordings](../concepts/logging-and-ingestion/apps-and-recordings.md)** - Managing application and recording IDs
- **[Visualizers and Overrides](../concepts/visualization/visualizers-and-overrides.md)** - Customizing rendering
- **[Chunks](../concepts/logging-and-ingestion/chunks.md)** - Internal storage mechanism (advanced)

## How-to guides

Practical guides for specific tasks and advanced features:

### Logging data
- **[Logging](../howto/logging-and-ingestion.md)** - Advanced logging techniques
- **[Send Columns](../howto/logging-and-ingestion/send-columns.md)** - Efficiently log columnar data
- **[Using Native Loggers](../howto/integrations/integrate-host-loggers.md)** - Integrate with existing logging systems
- **[Short-lived Entities](../howto/logging-and-ingestion/clears.md)** - Handling temporary data

### Visualization
- **[Visualization](../howto/visualization.md)** - Advanced visualization techniques
- **[Configure Viewer Through Code](../getting-started/configure-the-viewer.md#programmatic-blueprints)** - Programmatic viewer configuration
- **[Fixed Window Plots](../howto/visualization/fixed-window-plot.md)** - Creating time-windowed plots

### Data management
- **[DataFrame API](../howto/query-and-transform/get-data-out.md)** - Query recordings programmatically
- **[Get Data Out](../howto/query-and-transform/get-data-out.md)** - Export data from Rerun
- **[MCAP Integration](../howto/logging-and-ingestion/mcap.md)** - Working with MCAP files
- **[Shared Recordings](../howto/logging-and-ingestion/shared-recordings.md)** - Collaborate with recordings

### Integration & deployment
- **[Integrations](../howto/integrations.md)** - Integrate Rerun with other tools
- **[Embed Rerun Viewer](../howto/integrations/embed-web.md)** - Embed the viewer in your application
- **[Jupyter Notebooks](../howto/integrations/embed-notebooks.md)** - Use Rerun in notebooks
- **[Callbacks](../howto/visualization/callbacks.md)** - Respond to viewer events

### Performance & optimization
- **[Limit RAM Usage](../howto/visualization/limit-ram.md)** - Control memory consumption
- **[Optimize Chunks](../howto/logging-and-ingestion/optimize-chunks.md)** - Fine-tune data storage

### Extending Rerun
- **[Extend](../howto/extend.md)** - Add custom types and visualizations

### Examples
- **[ROS2 Nav Turtlebot](../howto/integrations/ros2-nav-turtlebot.md)** - Complete robotics example

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
