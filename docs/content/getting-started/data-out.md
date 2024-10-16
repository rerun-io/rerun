---
title: Get data out of Rerun
order: 700
---

At its core, Rerun is a database. The viewer includes the [dataframe view](../reference/types/views/dataframe_view) to explore data in tabular form, and the SDK includes an API to export the data as dataframes from the recording. These features can be used, for example, to perform analysis on the data and log back the results to the original recording.

In this three-part guide, we explore such a workflow by implementing an "open jaw detector" on top of our [face tracking example](https://rerun.io/examples/video-image/face_tracking). This process is split into three steps:

1. [Explore a recording with the dataframe view](data-out/explore-as-dataframe)
2. [Export the dataframe](data-out/export-dataframe)
3. [Analyze the data and log the results](data-out/analyze-and-log)

Note: this guide uses [Pandas](https://pandas.pydata.org) dataframes because of how popular this package is. The same concept however applies in the same way for alternative dataframe packages such as [Polars](https://pola.rs).

If you just want to see the final result, jump to the [complete script](data-out/analyze-and-log.md#complete-script) at the end of the third section.
