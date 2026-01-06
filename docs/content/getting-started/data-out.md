---
title: Query and Transform
order: 450
---

At its core, Rerun is a database. The OSS server is our small-scale in-memory parallel to our commercial cloud offering.

In this three-part guide, we explore a query workflow by implementing an "open jaw detector" on top of our [face tracking example](https://rerun.io/examples/video-image/face_tracking). This process is split into three steps:

1. [Explore a recording with the dataframe view](data-out/explore-as-dataframe.md)
2. [Export the dataframe](data-out/export-dataframe.md)
3. [Analyze the data and send back the results](data-out/analyze-and-send.md)

Note: this guide uses the popular [Pandas](https://pandas.pydata.org) dataframe package. The same concept however applies in the same way for alternative dataframe packages such as [Polars](https://pola.rs) or using [Datafusion](https://datafusion.apache.org/python/) directly.

If you just want to see the final result, jump to the [complete script](data-out/analyze-and-send.md#complete-script) at the end of the third section.
