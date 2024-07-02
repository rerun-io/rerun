---
title: Roadmap
order: 200
---
Rerun is building a data management and visualization engine for multimodal data that changes over time.
We aim to make it fast, and easy to use, and easy to adapt and integrate into your existing workflows.

Open an issue or pull request on [GitHub](https://github.com/rerun-io/rerun) or join us on [Discord](https://discord.gg/PXtCgFBSmH) to let the community know what you'd like to see.


This page is meant to give an high level overview of ongoing and planned work.

## We continually work on
- Performance improvements
- UX & DX improvements
- Supporting more data types

## Roadmap of major feature areas

### Early July 2024: release 0.17
- Blueprint component overrides & defaults (from code & UI)
- Redesigned selection view to make inspecting and editing visualization options easier
- Improved experience for embedding Rerun in web pages
- Improved notebook experience, including live visualizations from running cells
- A lot more blueprint configurability through code (still Python only)

### Near term: summer 2024
- Time-batch api: the ability to insert large chunks of e.g. time series in a single call
- Improved ingestion performance for large sets of small data (time series)
- Query API for reading back data from the SDK
- Improved support for working directly with Arrow data and the Rerun extension types
- A generic multimodal table view

### Medium term (Q3-4 2024)
- Audio and maps support
- Decoding h264 video in the viewer
- Official ROS2 bridge
- Callbacks and the ability to build interactive applications with Rerun
    - For example: UI for tweaking configs, custom data annotation tools, etc
- Rerun data platform (commercial)
    - Get in touch on hi@rerun.io if you're interested in becoming a design partner

### Longer term
- Extensibility of all parts of the stack
- Data format stability
