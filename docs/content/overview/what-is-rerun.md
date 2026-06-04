---
title: The Data Layer for Physical AI
order: 0
---

Rerun covers the whole journey from raw recordings to training, on a single unified data layer for multi-rate, multimodal robotics data.

It's comprised of **Rerun SDK**: an open source library and tools for logging, storing, querying, visualizing, and training on multi-rate, multimodal data; and
**Rerun Hub**: a data catalog and backend for large scale storage, access, and streaming of robotics data from object storage.

## The problem

Building intelligent physical systems requires rapid iteration on both data and models. But teams often get stuck because:

- Data from sensors arrives at different rates and in different formats
- Understanding what went wrong requires visualizing multimodal data (images, point clouds, sensor readings) together in time
- Extracting, cleaning, and preparing data for training involves too many manual steps
- Switching between different tools for each step slows everything down

The best robotics teams minimize their time from new data to training. Rerun gives you the unified infrastructure to make that happen.

## Who is Rerun for?

Rerun is built for teams developing intelligent physical systems:

- **Robotics engineers** debugging perception, controls, and planning
- **Perception teams** analyzing sensor data and model outputs
- **ML engineers** preparing datasets and understanding model behavior
- **Autonomy teams** developing and testing decision-making systems

If you're working with robots, drones, autonomous vehicles, spatial AI, or any system with data that evolves over time, Rerun helps you move faster.

## How do you use it?

### Log and ingest
Use the [logging API](../getting-started/data-in.md) to log multimodal data from your code, or [the chunk processing API](../concepts/logging-and-ingestion/chunk-processing-api.md) to convert your existing data to the [.rrd](../concepts/logging-and-ingestion/rrd-format.md) file format to later visualize or query.
<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/d1c242b745000b3dbba0dc42a861e2e6b760d614_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/0aa33c2c86855ee06122e761562ca31e82486ad6_d2-light.svg" alt="">
</div>

### Visualize
Rerun provides an open source pre-built [viewer](../reference/viewer/overview.md) that is [adjustable](../getting-started/configure-the-viewer.md) and [extensible](../howto/extend.md).
You can log directly to the viewer, [open](../getting-started/data-in/open-any-file.md) a range of file formats to get data into the viewer, or even connect the viewer to a Rerun [catalog](../concepts/query-and-transform/catalog-object-model.md).

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/2ded478d0e3d66b8532f1b9991ed786a2919d6d7_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/b7baf766ffcb27babd2866ebc05f016e8da53111_d2-light.svg" alt="">
</div>

### Query and transform
The Rerun file format supports both high performance visualization and querying over the same data source.

You can use the open source [catalog](../concepts/query-and-transform/catalog-object-model.md) server for running local [laptop scale examples](../getting-started/data-out).
We also offer **Rerun Hub**, a scalable catalog for robotic data, for teams that need collaborative dataset management, version control, and cloud storage ([reach out](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ?typeform-source=docs) to learn more).
These are API compatible so the only difference from our examples to **Rerun Hub** is that you connect to an existing server instead of launching your own.

#### Prepare catalog
Before querying or viewing recordings on the catalog we have to register them.
We group recordings as [datasets](../concepts/query-and-transform/catalog-object-model.md#datasets).
Since Rerun indexes existing data in place, registration needs paths to RRDs to index: in object store for **Rerun Hub** or on disk for local catalog server.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/40ada7a9f3834208554b8de80a0bab3fb8e5f108_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/63998cd168884dbe92c585650d2d8cdcdff826fc_d2-light.svg" alt="">
</div>

#### Use catalog
At this point a viewer can connect to the prepared catalog or we show the basic steps to perform a query.
We specify what dataset we want to query, get access to a lazy loaded [dataframe](../concepts/query-and-transform/dataframe-queries.md), specify our query, and retrieve the results.
Queries can be specified with SQL or dataframe APIs allowing the flexibility to investigate anything about your data.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/4bfd86b4605405d018ecd44d8d8062114126a952_d2.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/6922fdbb9c24357ea3a585be62d6ccb572a70350_d2-light.svg" alt="">
</div>

### Train
Use the catalog as a data source for [training](../getting-started/train.md): a dataloader runs a query against the catalog and yields training batches.

<div class="d2-diagram">
  <img class="d2-dark" src="https://static.rerun.io/15a375cfcc03a73d74acdec06b5f36c43e988dfe_d2-dark.svg" alt="">
  <img class="d2-light" src="https://static.rerun.io/75c7bcab918f7067cae09eb665cdec4d3d466e70_d2-light.svg" alt="">
</div>

## Get started

Ready to speed up your iteration cycle?

- [Quick start guide](../getting-started.md) - Get up and running in minutes
- [Examples](https://rerun.io/examples) - See Rerun in action with real data
- [Concepts](../concepts.md) - Learn how Rerun works under the hood

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- [Submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project
