---
title: Roadmap
order: 1
---
Rerun is building a visualization engine for streams of multimodal data that's easy to use, adapt, and extend.

Open an issue or pull request on [GitHub](https://github.com/rerun-io/rerun) or join us on [Discord](https://discord.gg/PXtCgFBSmH) to let the community know what you'd like to see.


This page is meant to give an high level overview of ongoing and planned work.

## We continually work on
- Performance improvements
- UX & DX improvements
- Supporting more data types

## Roadmap of major feature areas

### Last week of November: Release 0.11
- Improvements to the C++ SDK
    - Even easier cmake build system integration
    - Logging non-rerun types is faster and easier
- Expand the "Visual History" feature to "Visible Time Range"
    - For supported views and data types you'll be able to specify both absolute and relative time ranges to include data from.

### December 2023: Release 0.12
- Significant performance improvements for all time range visualizations, particularly time series.

### Near term: Now - Q1 2024
- End to end performance for high frequency time series logging
- Layout and configuration from code (blueprint)
- Datasets that are bigger than RAM for the native viewer

### Medium term
- Extensibility of all parts of the stack
- Easily read query data recordings from user code
- Callbacks and the ability to build interactive applications with Rerun
    - For example: UI for tweaking configs, custom data annotation tools, etc
- Data format stability
