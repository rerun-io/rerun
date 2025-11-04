---
title: What is Rerun?
order: 0
---

Rerun is a data platform for Physical AI that helps you understand and improve complex processes involving rich multimodal data like 2D, 3D, text, time series, and tensors.

It combines simple and flexible data logging with a powerful visualizer and query engine, designed specifically for domains like robotics, spatial computing, embodied AI, computer vision, simulation, and any system involving sensors and signals that evolve over time.

## The Rerun data platform

Rerun provides an integrated solution for working with multimodal temporal data:

**Time-aware data model:** At its core is an [Entity Component System (ECS)](../concepts/entity-component.md) designed for robotics and XR applications. This model understands both [spatial relationships](../concepts/spaces-and-transforms.md) and [temporal evolution](../concepts/timelines.md), making it natural to work with sensor data, transforms, and time-series information.

**Built-in visualization:** A fast, embeddable visualizer lets you see your data as 3D scenes, images, plots, and text—all synchronized and explorable through time. Build [layouts and customize visualizations](../getting-started/configure-the-viewer.md) interactively or [programmatically](../concepts/blueprint.md).

**Query and export:** Extract clean [dataframes](../howto/dataframe-api.md) for analysis in Pandas, Polars, or DuckDB. Use recordings to create datasets for training and evaluating your models.

**Flexible ingestion:** Load data from your code via the [SDK](../getting-started/quick-start.md), from storage formats like [MCAP](../howto/mcap.md), or from proprietary log formats. [Extend Rerun](../howto/extend.md) when you need custom types or visualizations.

## What is Rerun for?

Rerun helps you debug, understand, and improve systems that generate rich multimodal data. Here's a concrete example:

### Example: Debugging a vacuum cleaning robot

Say you're building a vacuum cleaning robot and it keeps running into walls. A traditional debugger won't help, and text logs aren't enough—the robot may log "Going through doorway" but that won't explain why it thinks a wall is a door.

What you need is to see the world from the robot's perspective in time:

* RGB camera feed
* Depth images
* Lidar scans
* Segmentation results (how the robot interprets what it sees)
* The robot's 3D map of the apartment
* Detected objects as 3D shapes in the map
* Confidence scores
* And more

You want to see how all these data streams evolve over time so you can pinpoint exactly what went wrong, when, and why.

Maybe a sun glare hit a sensor wrong, confusing the segmentation network and leading to bad object detection. Or a bug in the lidar code. Or broken odometry made the robot think it was somewhere else. Rerun helps you find out!

But visual debugging is just the start. Seeing your data also:
- Gives you ideas for algorithm improvements
- Helps identify new test cases to set up
- Suggests datasets to collect
- Lets you explain your system to colleagues and stakeholders

And the same data you use for visualization can be [queried](../getting-started/data-out.md) to create clean datasets for training and evaluating your models.

## How do you use it?

<picture>
  <img src="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1200w.png">
</picture>

1. Use the [Rerun SDK](../getting-started/quick-start.md) to [log multimodal data](../getting-started/data-in.md) from your code or load it from storage
2. View live or recorded data in the standalone viewer or [embedded in your app](../howto/embed-rerun-viewer.md)
3. Build layouts and [customize visualizations](../getting-started/configure-the-viewer.md) interactively in the UI or through the SDK
4. [Query recordings](../howto/dataframe-api.md) to get clean dataframes into tools like Pandas, Polars, or DuckDB
5. [Extend Rerun](../howto/extend.md) when you need to

## How does it work?
That's a big question for a welcome page. The short answer is that
Rerun goes to extreme lengths to make handling and visualizing
multimodal data streams easy and performant. Learn more in our [concepts documentation](../concepts.md).

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- Or [submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project

