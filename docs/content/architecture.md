---
title: Architecture
order: -1
---

## Overview

Rerun provides a fast and extensible data visualization infrastructure for all kinds of data: 2D, 3D, timeseries, and any permutation thereof.

It is designed to be fast, flexible, extensible, and easily integrated anywhere.

The Rerun Viewer can be compiled to Wasm, allowing it to [run in a browser](https://demo.rerun.io/version/0.8.1) and be embedded anywhere you can put a web-view [(e.g. in Jupyter Notebook)](https://colab.research.google.com/drive/1R9I7s4o6wydQC_zkybqaSRFTtlEaked_).

Use one of out logging SDKs to produce log data that is then either live-streamed to the Rerun Viewer, or stored in a file for later viewing.

Rerun is open-source, being built in the open on GitHub.

## Architecture Overview

*Message: we address each of our solution’s building bocks in a thorough manner, laying ground infrastructure for any future needs.*

**SDK:** you can log using C++, Python and Rust. The bulk of our SDKs are code-generated from a simple [IDL](https://en.wikipedia.org/wiki/IDL_(programming_language)), making it easy to extend in the future. The SDK produces [Arrow](https://arrow.apache.org/overview/) data.

The **Communication Layer** abstract the transport layers and enables the Rerun Viewer to ingest data from various sources, including files, TCP, and WebSocket.

The **Data Layer** provides a queryable store for arbitrary structured time-dependant data.

The **Vizualisation Layer** is made of composable, extensible collection building blocks covering 2D and 3D renderers, plotting widgets, and textual data display. We have written our own high-level renderer (link: re_rendered) on top of wgpu [LINK: wgpu anchor below]

**GUI:** We use [egui](https://www.egui.rs/) (made by our CTO), an easy-to-use [immediate mode GUI](https://github.com/emilk/egui#why-immediate-mode).

**3D Rendering:** We have written our own high-level renderer `[re_renderer](notion://www.notion.so/rerunio/crates/re_renderer/README.md)` on top of `wgpu`.

Data can be logged using C++, Python and Rust. The bulk of our SDKs are code-generated from a simple [IDL](https://en.wikipedia.org/wiki/IDL_(programming_language)), making it easy to extend in the future. The SDK produces [Arrow](https://arrow.apache.org/overview/) data.

Users can extend the builtin datatypes by logging arbitrary Arrow data.

Data is then stored and queried from a custom datastore built specifically to match our data model (timeseries of arbitrarily complex Arrow data).

Internally, the data model is implemented as a special purpose entity-component-system where time is a first-class citizen.

The Rerun viewer handles all visualization tasks. It is built atop egui (made by our CTO), an easy-to-use immediate mode GUI as well as `re_renderer`, a purpose-built 2D & 3D renderer built on top of `wgpu`.

Users can extend the Viewer with arbitrary visualizations of their own making.

## Extensibility

## Technology stack

*Message: show future-proofness by showing strong foundations and strong reasons to use them.*

- [Apache Arrow](https://arrow.apache.org)
- **Rust**: the only mainstream language that is both fast and safe. https://www.rerun.io/blog/why-rust
- We use [WebAssembly](https://webassembly.org) to get the viewer running at high speeds inside a browser or anywhere you can embed a web-view. For the native viewer we compile natively (no need for Electron!)
- [egui](https://www.egui.rs)
- [wgpu](https://wgpu.rs)
