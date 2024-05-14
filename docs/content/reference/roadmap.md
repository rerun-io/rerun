---
title: Roadmap
order: 200
---
Rerun is building a visualization engine for streams of multimodal data that's easy to use, adapt, and extend.

Open an issue or pull request on [GitHub](https://github.com/rerun-io/rerun) or join us on [Discord](https://discord.gg/PXtCgFBSmH) to let the community know what you'd like to see.


This page is meant to give an high level overview of ongoing and planned work.

## We continually work on
- Performance improvements
- UX & DX improvements
- Supporting more data types

## Roadmap of major feature areas

### Early April 2024: release 0.15
- Layout and viewport content from code (blueprint part 1)
- Data-loader plugins callable from the SDK
- Linux ARM64 support in pre-built artifacts

### Near term: now - end of Q2 2024
- Property overrides from code (blueprint part 2)
    - Includes setting visible time range from code
- Broader coverage of robotics and spatial computing data types
- Extension mechanisms for dynamically reading from external storage
    - For example files like: VRS, MCAP, or mp4
    - Also brings support for datasets that are bigger than RAM in the native viewer

### Medium term (Q3-4 2024)
- Make Rerun easier to use when training and evaluating ML models
- Deeper support for modalities like text and audio
- Callbacks and the ability to build interactive applications with Rerun
    - For example: UI for tweaking configs, custom data annotation tools, etc

### Longer term
- Extensibility of all parts of the stack
- Easily query data recordings from user code
- Data format stability
