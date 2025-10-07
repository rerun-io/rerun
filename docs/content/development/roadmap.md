---
title: Roadmap
order: 0
---
Rerun is building a data management and visualization engine for multimodal data that changes over time.
We aim to make it fast, simple to use, and easy to adapt and integrate into your existing workflows.

Open an issue or pull request on [GitHub](https://github.com/rerun-io/rerun) or join us on [Discord](https://discord.gg/PXtCgFBSmH) to let the community know what you'd like to see. Or if you're open for a conversation, [sign up here](https://rerun.io/feedback).


This page is meant to give an high level overview of ongoing and planned work. This roadmap is subject to change; GitHub will be the most authoritative source for active development.

## We continually work on
- Performance improvements
- UX & DX improvements
- Supporting more data types
- Rerun Cloud features (commercial)
    - Get in touch on hi@rerun.io if you're interested in becoming a design partner

## Roadmap of major feature areas

### Near term
- Improving our data ingestion and interpretation flexibility, especially through initial support for common ROS2 messages in MCAP files
- Greater capabilities around sharing links to data
- h.265 video streaming support
- An in-memory catalog to make recording file management simpler

### Medium term
- Filtering in table and dataframe views
- Configurable data interpretability (e.g. MCAP files with custom messages)
    - Including _data blueprints_ that define and store interpretations for later viewing
- Dataset views that give zero-copy modified views into large datasets

### Longer term
- Callbacks and the ability to build interactive applications with Rerun
    - For example: UI for tweaking configs, custom data annotation tools, etc
- Official ROS2 bridge
- Extensibility of all parts of the stack
- Data format stability
