---
title: What is Rerun?
order: 0
---

Rerun is the Unified Data Layer for Physical AI that helps you build smarter robots.

Rerun is comprised of **Rerun SDK**: an open source library and tools for logging, storing, querying, visualizing, and training on multi-rate, multimodal data; and
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
```d2
direction: right

generator: "data generator" {
}

existing: "Existing data\n\n• MCAP\n• LeRobot\n• etc." {
  shape: page
}

conversion: "converter" {
}

rrd: ".RRD" {
  shape: page
  width: 220
  height: 420
}

generator -> rrd
existing -> conversion
conversion -> rrd
```

### Visualize
Rerun provides an open source pre-built [viewer](../reference/viewer/overview.md) that is [adjustable](../getting-started/configure-the-viewer.md) and [extensible](../howto/extend.md).
You can log directly to the viewer, [open](../getting-started/data-in/open-any-file.md) a range of file formats to get data into the viewer, or even connect the viewer to a Rerun [catalog](../concepts/query-and-transform/catalog-object-model.md).

```d2
direction: right

generator: "data generator" {
  width: 220
  height: 110
}

rrd: ".RRD" {
  shape: page
  width: 220
  height: 110
  style.font-size: 28
}

formats: "Supported formats\n\n• MCAP\n• LeRobot\n• etc." {
  shape: page
  width: 300
  height: 260
  style.font-size: 20
}

importers: "Automated\nimporters" {
  width: 220
  height: 110
  style.font-size: 24
}

catalog: "Rerun Catalog" {
  shape: cylinder
  width: 220
  height: 130
  style.font-size: 24
}

viewer: "Rerun Viewer" {
  width: 600
  height: 500
  label.near: top-center
  style.font-size: 32

  big_view: "" {
    shape: rectangle
    width: 480
    height: 380
    style.font-size: 80
    style.fill: "#dddddd"
  }
}

generator -> viewer
rrd -> viewer
formats -> importers
importers -> viewer
catalog -> viewer
```

### Query and transform
The Rerun file format supports both high performance visualization and querying over the same data source.

You can use the open source [catalog](../concepts/query-and-transform/catalog-object-model.md) server for running local [laptop scale examples](../getting-started/data-out).
We also offer **Rerun Hub**, a scalable catalog for robotic data, for teams that need collaborative dataset management, version control, and cloud storage ([reach out](https://5li7zhj98k8.typeform.com/to/a5XDpBkZ?typeform-source=docs) to learn more).
These are API compatible so the only difference from our examples to **Rerun Hub** is that you connect to an existing server instead of launching your own.

#### Prepare catalog
Before querying or viewing recordings on the catalog we have to register them.
We group recordings as [datasets](../concepts/query-and-transform/catalog-object-model.md#datasets).
Since Rerun indexes existing data in place, registration needs paths to RRDs to index: in object store for **Rerun Hub** or on disk for local catalog server.

```d2
direction: right

create: "1. create_dataset"
handle: "2. dataset handle"
register: "3. handle.register"

catalog: "Rerun Catalog" {
  shape: cylinder
  height: 400
}

create -> catalog: "dataset name"
handle <- catalog: "returns"
register -> catalog: "path to .rrd(s)"
```

#### Use catalog
At this point a viewer can connect to the prepared catalog or we show the basic steps to perform a query.
We specify what dataset we want to query, get access to a lazy loaded [dataframe](../concepts/query-and-transform/dataframe-queries.md), specify our query, and retrieve the results.
Queries can be specified with SQL or dataframe APIs allowing the flexibility to investigate anything about your data.

```d2
direction: right

handle: "1. dataset handle"
df1: "2. DataFrame"
df2: "3. DataFrame"
result: "4. query result" {
  shape: page
}

catalog: "Rerun Catalog" {
  shape: cylinder
  height: 500

  my_dataset: "my_dataset" {
    segment_a
    segment_b
  }
}

handle -> catalog: |md `dataset.reader(index=…)`|
df1 <- catalog: "returns"
df2 -> catalog: |md `df.select(…).where(…)`|
result <- catalog: "returns"
```

## Get started

Ready to speed up your iteration cycle?

- [Quick start guide](../getting-started.md) - Get up and running in minutes
- [Examples](https://rerun.io/examples) - See Rerun in action with real data
- [Concepts](../concepts.md) - Learn how Rerun works under the hood

## Can't find what you're looking for?

- Join us in the [Rerun Community Discord](https://discord.gg/xwcxHUjD35)
- [Submit an issue](https://github.com/rerun-io/rerun/issues) in the Rerun GitHub project
