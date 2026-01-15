---
title: What is Rerun?
order: 0
---

Rerun is a data platform for Physical AI that helps you understand and improve complex processes involving rich multimodal data like 2D, 3D, text, time series, and tensors.

It combines simple and flexible data logging with a powerful visualizer and query engine, designed specifically for domains like robotics, spatial computing, embodied AI, computer vision, simulation, and any system involving sensors and signals that evolve over time.

## The problem

Building intelligent physical systems requires rapid iteration on both data and models. But teams often get stuck because:

- Data from sensors arrives at different rates and in different formats
- Understanding what went wrong requires visualizing multimodal data (images, point clouds, sensor readings) together in time
- Extracting, cleaning, and preparing data for training involves too many manual steps
- Switching between different tools for each step slows everything down

The best robotics teams minimize their time from new data to training. Rerun gives you the unified infrastructure to make that happen.

## The Rerun data platform

Rerun provides an integrated solution for working with multimodal temporal data:

**Time-aware data model:** At its core is an [Entity Component System (ECS)](../concepts/logging-and-ingestion/entity-component.md) designed for robotics and XR applications. This model understands both [spatial relationships](../concepts/logging-and-ingestion/transforms.md) and [temporal evolution](../concepts/logging-and-ingestion/timelines.md), making it natural to work with sensor data, transforms, and time-series information.

**Built-in visualization:** A fast, embeddable visualizer lets you see your data as 3D scenes, images, plots, and text—all synchronized and explorable through time. Build [layouts and customize visualizations](../getting-started/configure-the-viewer.md) interactively or [programmatically](../concepts/visualization/blueprints.md).

**Query and export:** Extract clean [dataframes](../howto/query-and-transform/get-data-out.md) for analysis in Pandas, Polars, or DuckDB. Use recordings to create datasets for training and evaluating your models.

**Flexible ingestion:** Load data from your code via the [SDK](../getting-started/data-in.md), from storage formats like [MCAP](../howto/logging-and-ingestion/mcap.md), or from proprietary log formats. [Extend Rerun](../howto/extend.md) when you need custom types or visualizations.

## Who is Rerun for?

Rerun is built for teams developing intelligent physical systems:

- **Robotics engineers** debugging perception, controls, and planning
- **Perception teams** analyzing sensor data and model outputs
- **ML engineers** preparing datasets and understanding model behavior
- **Autonomy teams** developing and testing decision-making systems

If you're working with robots, drones, autonomous vehicles, spatial AI, or any system with sensors that evolve over time, Rerun helps you move faster.

## What Rerun is not

To set clear expectations:

- **Not a training platform**: Use Rerun with PyTorch, TensorFlow, JAX, etc. We prepare your data; you train your models.
- **Not a deployment tool**: Rerun helps you develop and understand your systems, not deploy them to production.
- **Not a robot operating system**: Rerun works with ROS, ROS2, or any other robotics stack.
- **Not a general visualization tool**: We're specialized for physical, multimodal, time-series data.

## How do you use it?

<picture>
  <img src="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/rerun-overview-new/1752fc259eef34f3aa8151b21b5937bc0bc2ad38/1200w.png">
</picture>

1. Use the [Rerun SDK](../getting-started/data-in.md) to [log multimodal data](../getting-started/data-in.md) from your code or load it from storage
2. View live or recorded data in the standalone viewer or [embedded in your app](../howto/integrations/embed-web.md)
3. Build layouts and [customize visualizations](../getting-started/configure-the-viewer.md) interactively in the UI or through the SDK
4. [Query recordings](../getting-started/data-out.md) to get clean dataframes into tools like Pandas, Polars, or DuckDB
5. [Extend Rerun](../howto/extend.md) when you need to

We also offer a commercial data platform for teams that need collaborative dataset management, version control, and cloud storage. [Learn more](https://rerun.io/pricing).

## Get started

Ready to speed up your iteration cycle?

- [Quick start guide](../getting-started.md) - Get up and running in minutes
- [Examples](https://rerun.io/examples) - See Rerun in action with real data
- [Concepts](../concepts.md) - Learn how Rerun works under the hood

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- [Submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project
