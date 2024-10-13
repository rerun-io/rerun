---
title: Get data out of Rerun
order: 700
---

At its core, Rerun is a database. The Rerun SDK includes an API to export data as dataframes from recording. This can be used, for example, to perform analysis on the data and even log back the results to the original recording.

In this series of articles, we explore this workflow by implementing an "open jaw detector" on top of our [face tracking example](https://rerun.io/examples/video-image/face_tracking). This process is split into three steps:

1. [Explore a recording with the dataframe view](data-out/explore-as-dataframe)
2. [Export the dataframe](data-out/export-dataframe)
3. [Analyze the data and log the results](data-out/analyze-and-log)

For the fast track, jump to the [complete script](data-out/analyze-and-log.md#complete-script) at the end of the third section.