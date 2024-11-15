---
title: Query data out of Rerun
order: 450
---

At its core, Rerun is a database. The viewer includes the [dataframe view](../reference/types/views/dataframe_view.md) to explore data in tabular form, and the SDK includes an API to export the data as dataframes from the recording. These features can be used, for example, to perform analysis on the data and send back the results to the original recording.

In this three-part guide, we explore such a workflow by implementing an "open jaw detector" on top of our [face tracking example](https://rerun.io/examples/video-image/face_tracking). This process is split into three steps:

1. [Explore a recording with the dataframe view](data-out/explore-as-dataframe.md)
2. [Export the dataframe](data-out/export-dataframe.md)
3. [Analyze the data and send back the results](data-out/analyze-and-send.md)

Note: this guide uses the popular [Pandas](https://pandas.pydata.org) dataframe package. The same concept however applies in the same way for alternative dataframe packages such as [Polars](https://pola.rs).

If you just want to see the final result, jump to the [complete script](data-out/analyze-and-send.md#complete-script) at the end of the third section.
