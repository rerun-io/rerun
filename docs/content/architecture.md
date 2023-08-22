---
title: Architecture
order: -1
---

## Overview

Rerun provides a fast and extensible data visualization infrastructure adaptable to all kinds of data (2D, 3D, time series, etc.) and targets (native and web).
Our stack is designed to be fast, flexible, extensible, and easily adaptable to any workflow, from local development to cloud-based deployment.

Use one of our logging SDKs to produce log data that is then either live-streamed to the Rerun Viewer, or stored in a file for later viewing.

<img src="https://github.com/rerun-io/rerun/assets/49431240/3a0e3b1f-aa71-4c19-84c4-f4626ce4499c" alt="overview" width="90%" />

The Rerun Viewer is written in [Rust](https://www.rust-lang.org), which we chose for its performance, its memory safety, and cross-platform compatibility.

For web targets, the Rerun Viewer is compiled to [WebAssembly](https://webassembly.org), allowing it to [run in a browser](https://demo.rerun.io/) with nearly no performeance compromise, and to be embedded anywhere you can put a web-view [(e.g. in Jupyter Notebook)](https://colab.research.google.com/drive/1R9I7s4o6wydQC_zkybqaSRFTtlEaked_).

Rerun is open-source, being built in the open [on GitHub](https://github.com/rerun-io/rerun).

## Architecture Overview


<img src="https://github.com/rerun-io/rerun/assets/49431240/0b9cd101-e3a5-400f-acf4-d85ef72e2406" width="500px"  alt="The Rerun stack"/>


Data is ingested using our Python and/or Rust **SDKs** (C++ coming soon).
The SDKs are code-generated from a simple [IDL](https://en.wikipedia.org/wiki/IDL_(programming_language)), making them easy to extend in the future.
Data is serialized in [Apache Arrow](https://arrow.apache.org/overview/) format for language-independent, cross-platform compatibility.

The **Communication Layer** abstract the transport layers and enables the Rerun Viewer to ingest data from various sources, including files, TCP, and WebSocket.

The **Data Layer** provides a queryable store for arbitrary structured time-dependant data.
Internally, the data model is implemented as a special purpose [entity-component-system](https://en.wikipedia.org/wiki/Entity_component_system) where time is a first-class citizen.

The **Visualization Layer** is made of composable, extensible collection building blocks covering 2D and 3D renderers, plotting widgets, and textual data display.
We have written our own high-level, purpose-built renderer on top of [wgpu](https://wgpu.rs), which offers high-performance cross-platform compatibility for native and web targets.

Our **GUI** is built upon our CTO's [egui](https://www.egui.rs/) [immediate mode UI](https://github.com/emilk/egui#why-immediate-mode) framework.
It's designed to be offer a reactive, user-friendly and customizable interface that's easy to deploy to both native and web targets.


## Extensibility

_TODO_

Users can extend the builtin datatypes by logging arbitrary Arrow data.


<!--
## Immediate mode
The Rerun Viewer uses an [immediate mode GUI](https://github.com/emilk/egui#why-immediate-mode), [`egui`](https://www.egui.rs/).
This means that each frame the entire GUI is being laid out from scratch.

In fact, the whole of the Rerun Viewer is written in an immediate mode style.
Each rendered frame it will query the in-RAM data store, massage the results, and feed it to the renderer.

The advantage of immediate mode is that is removes all state management.
There is no callbacks that are called when some state has already changed, and the state of the blueprint is always in sync with what you see on screen.

Immediate mode is also a forcing function, pressuring us to relentlessly optimize our code.
This leads to a very responsive GUI, where there is no "hickups" when switching data source or doing time scrubbing.

Of course, this will only take us so far.
In the future we plan on caching queries and work submitted to the renderer so that we don't perform unnecessary work each frame.
We also plan on doing larger operation in background threads.
This will be necessary in order to support viewing large datasets, e.g. several million points.
The plan is still to do so within an immediate mode framework, retaining most of the advantages of stateless code.
-->
