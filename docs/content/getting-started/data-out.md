---
title: Query and Transform
order: 450
---

At its core, Rerun is a database.
The OSS server is a local catalog that keeps catalog metadata in memory and reads data from RRDs with a footer on demand.
It can therefore serve datasets larger than RAM, while Rerun Hub provides managed persistent storage for production use.

In this three-part guide, we explore a query workflow by implementing an "open jaw detector" on top of our [face tracking example](https://rerun.io/examples/video-image/face_tracking). This process is split into three steps:

1. [Explore a recording with the dataframe view](data-out/explore-as-dataframe.md)
2. [Export the dataframe](data-out/export-dataframe.md)
3. [Analyze the data and send back the results](data-out/analyze-and-send.md)

> [!NOTE]
> This guide uses the popular [Pandas](https://pandas.pydata.org) dataframe package. The same concept however applies for alternative dataframe packages such as [Polars](https://pola.rs) or using [Datafusion](https://datafusion.apache.org/python/) directly.

If you just want to see the final result, jump to the [complete script](data-out/analyze-and-send.md#complete-script) at the end of the third section.
