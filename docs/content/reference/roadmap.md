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

### Early January 2024: Release 0.12
- Parallelized rendering and processing for all views
- Plugin system for loading any file into Rerun

### Near term: Now - Q1 2024
- End to end performance for high frequency time series logging
- Layout and configuration from code (blueprint)
- Datasets that are bigger than RAM for the native viewer
- CLI for manipulating and exporting data from rrd files

### Medium term (Q2-3 2024)
- Broader coverage of robotics data types
- Extension mechanisms for dynamically reading from external storage
    - For example files like: VRS, MCAP, or mp4
- Callbacks and the ability to build interactive applications with Rerun
    - For example: UI for tweaking configs, custom data annotation tools, etc

### Longer term
- Extensibility of all parts of the stack
- Easily query data recordings from user code
- Data format stability
