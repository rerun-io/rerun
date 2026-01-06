---
title: Docs Guide
order: 400
---

This page provides an overview of how the Rerun documentation is organized to help you find what you need.

## Overview

High-level introduction to Rerun:

- **[What is Rerun?](what-is-rerun.md)** - Learn about Rerun's data platform for Physical AI
- **[Installing the Viewer](installing-viewer.md)** - Get Rerun installed on your system

## Getting started

Step-by-step guides to get up and running quickly:

- **[Quick Start](../getting-started/quick-start.md)** - Choose your language (Python, Rust, C++) and create your first visualization
- **[Data In](../getting-started/data-in.md)** - Learn how to log data to Rerun from your code
- **[Data Out](../getting-started/data-out.md)** - Query and export data from Rerun recordings
- **[Configure the Viewer](../getting-started/configure-the-viewer.md)** - Customize the visualization to your needs
- **[Troubleshooting](../getting-started/troubleshooting.md)** - Solutions to common issues

## Concepts

Understanding the foundational concepts behind Rerun:

- **[Application Model](../concepts/app-model.md)** - How Rerun applications are structured
- **[Entity Component System](../concepts/entity-component.md)** - Rerun's data model
- **[Entity Paths](../concepts/entity-path.md)** - Organizing your data hierarchically
- **[Spaces and Transforms](../concepts/spaces-and-transforms.md)** - Working with coordinate systems
- **[Timelines](../concepts/timelines.md)** - Managing temporal data
- **[Blueprints](../concepts/blueprint.md)** - Configuring visualization layouts
- **[Batches](../concepts/batches.md)** - Efficiently logging collections of data
- **[Static Data](../concepts/static.md)** - Data that exists across all timelines
- **[Query Semantics](../concepts/latest-at.md)** - How Rerun resolves data queries
- **[Annotation Context](../concepts/annotation-context.md)** - Shared styling and labels
- **[Apps and Recordings](../concepts/apps-and-recordings.md)** - Managing application and recording IDs
- **[Visualizers and Overrides](../concepts/visualizers-and-overrides.md)** - Customizing rendering
- **[Chunks](../concepts/chunks.md)** - Internal storage mechanism (advanced)

## How-to guides

Practical guides for specific tasks and advanced features:

### Logging data
- **[Logging](../howto/logging.md)** - Advanced logging techniques
- **[Send Columns](../howto/send_columns.md)** - Efficiently log columnar data
- **[Using Native Loggers](../howto/using-native-loggers.md)** - Integrate with existing logging systems
- **[Short-lived Entities](../howto/short-lived-entities.md)** - Handling temporary data

### Visualization
- **[Visualization](../howto/visualization.md)** - Advanced visualization techniques
- **[Configure Viewer Through Code](../howto/configure-viewer-through-code.md)** - Programmatic viewer configuration
- **[Fixed Window Plots](../howto/fixed-window-plot.md)** - Creating time-windowed plots

### Data management
- **[DataFrame API](../howto/dataframe-api.md)** - Query recordings programmatically
- **[Get Data Out](../howto/get-data-out.md?speculative-link)** - Export data from Rerun
- **[MCAP Integration](../howto/mcap.md)** - Working with MCAP files
- **[Shared Recordings](../howto/shared-recordings.md)** - Collaborate with recordings

### Integration & deployment
- **[Integrations](../howto/integrations.md)** - Integrate Rerun with other tools
- **[Embed Rerun Viewer](../howto/embed-rerun-viewer.md)** - Embed the viewer in your application
- **[Jupyter Notebooks](../howto/notebook.md)** - Use Rerun in notebooks
- **[Callbacks](../howto/callbacks.md)** - Respond to viewer events

### Performance & optimization
- **[Limit RAM Usage](../howto/limit-ram.md)** - Control memory consumption
- **[Optimize Chunks](../howto/optimize-chunks.md)** - Fine-tune data storage

### Extending Rerun
- **[Extend](../howto/extend.md)** - Add custom types and visualizations

### Examples
- **[ROS2 Nav Turtlebot](../howto/ros2-nav-turtlebot.md)** - Complete robotics example

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
