---
title: Roadmap
order: 0
description: Planned work on Rerun, and features that are still missing
---
Rerun is building a data management and visualization engine for multimodal data that changes over time.
We aim to make it fast, simple to use, and easy to adapt and integrate into your existing workflows.

Open an issue or pull request on [GitHub](https://github.com/rerun-io/rerun) or join us on [Discord](https://discord.gg/Gcm8BbTaAj) to let the community know what you'd like to see.
Or if you're open for a conversation, [sign up here](https://rerun.io/feedback).

This page gives a high-level overview of ongoing and planned work.

## We continually work on
- Performance improvements
- UX & DX improvements
- Supporting more data types
- Rerun Hub features (commercial)
    - Get in touch on hi@rerun.io if you're interested in becoming a design partner

## Roadmap of major feature areas

### Near term
- The agent panel: chat with a coding agent that can inspect and drive the Viewer through the [Rerun MCP server](../viewer/extend/mcp.md)
    - You can try it today: click the agent button in the top right corner of the Viewer
    - We are working on making it a stable, non-experimental feature
- A refreshed look and feel for the Viewer
- Greater capabilities around sharing links to data

### Medium term
- Better blueprint management, such as several blueprints per dataset
- A composable Viewer toolkit, for building your own viewer from Rerun's UI building blocks
- More mature data loaders, and more ready-made conversions of open robotics datasets
- Richer filtering in the dataframe view (table views already have [filters](../viewer/blueprints-and-views/configure-table-blueprints.md#filtering-rows))
- Dataset views that give zero-copy modified views into large datasets

### Longer term
- Interactive applications built with Rerun, such as UI for tweaking configs or custom data annotation tools
    - Today you can [react to Viewer events](../viewer/extend/callbacks.md) in the web Viewer, in notebooks, and from Rust, but the native Viewer has no user widgets or callbacks for Python
- Official ROS 2 bridge
    - Today you can [import ROS 2 bags in MCAP format](../logging-and-ingestion/import/mcap/working-with-mcap.md) and re-log live topics yourself, as shown in the [ROS 2 example](../integrations/ros2-nav-turtlebot.md)
- Extensibility of all parts of the stack

### Recently shipped
- Configurable data interpretation, for example MCAP files with custom messages: see [Lenses](../query-and-transform/lenses.md) and [component mappings](../viewer/blueprints-and-views/component-mappings.md)
- Callbacks for Viewer events: see [React to events in the Viewer](../viewer/extend/callbacks.md)
- Driving the Viewer from code: `rerun.experimental.ViewerClient` in Python and the `WebViewer` API in JavaScript
- Data format stability: the `.rrd` framing is stable, and recordings from Rerun 0.23 and later are migrated automatically on load (see [RRD format stability](../logging-and-ingestion/chunks-and-rrd/rrd-format.md#stability))

## Missing features

Rerun can't do everything yet.
This section lists features that users often ask for, with the best workaround we know of.

- If an item links to an open GitHub issue, give it a 👍. That is how we prioritize.
- If your missing feature is not listed, or has no issue, [open one](https://github.com/rerun-io/rerun/issues/new/choose).

### Viewer
- User widgets (buttons, sliders) and callbacks into Python from the native Viewer. Workaround: use [callbacks](../viewer/extend/callbacks.md) in the web Viewer, in notebooks, or from Rust, or [extend the Viewer in Rust](../viewer/extend/extend-ui.md).
    - We are working on adding web views to the Viewer.
    - [#2691](https://github.com/rerun-io/rerun/issues/2691), [#3087](https://github.com/rerun-io/rerun/issues/3087)
- Reading the current selection from Python: `rerun.experimental.ViewerClient.get_state()` has no selection. Workaround: selection events in the web Viewer and notebooks.
    - [#12990](https://github.com/rerun-io/rerun/issues/12990)
- Locking the 3D camera against user input, or limiting zoom and orbit.
    - [#2497](https://github.com/rerun-io/rerun/issues/2497)
- Linking or highlighting different entities across views. Only the same entity path and instance link today.
    - [#8866](https://github.com/rerun-io/rerun/issues/8866), [#4379](https://github.com/rerun-io/rerun/issues/4379)
- Setting the initial time cursor in a blueprint (`TimePanel` sets timeline, play state, and time selection, but not the cursor). Workaround: the `#when=` fragment of a [Viewer URL](../viewer/extend/urls.md), or `set_time` on `rerun.experimental.ViewerClient`.
- A scrollable container for many views. Container kinds are only tabs, horizontal, vertical, and grid. Workaround: put views in tabs.
    - [#6627](https://github.com/rerun-io/rerun/issues/6627)
- Hiding entities by default at log time: `EntityBehavior` only has an effect in the blueprint. Workaround: set `EntityBehavior(visible=False)` as an override in your blueprint.
    - [#6541](https://github.com/rerun-io/rerun/issues/6541)

### Plots
- Logarithmic axis scale. Workaround: log the `log10` of your values.
    - [#6773](https://github.com/rerun-io/rerun/issues/6773)
- Arbitrary X axis (XY plots) in the time series view. Workaround: log `LineStrips2D` or `Points2D` to a 2D view.
    - [#8286](https://github.com/rerun-io/rerun/issues/8286), [#7346](https://github.com/rerun-io/rerun/issues/7346)
- Waterfall view of one vector per time step. Workaround: log the vector as a `BarChart` each time step, or build an `Image` from the history yourself.
    - [#10130](https://github.com/rerun-io/rerun/issues/10130)
- Text search in the text log view. Today you can only filter by log level.
    - [#12890](https://github.com/rerun-io/rerun/issues/12890)
- User-defined columns in the text log view. Workaround: put the extra fields in the log body.

### Labels
- Font size of labels.
    - [#4228](https://github.com/rerun-io/rerun/issues/4228)
- Text fixed to the screen as an overlay on a 2D or 3D view. Workaround: show the text in a `TextDocument` view next to it, or attach a label to a point.

### 3D rendering
- Shadows, user-placed lights, and materials beyond albedo.
    - [#4845](https://github.com/rerun-io/rerun/issues/4845), [#1800](https://github.com/rerun-io/rerun/issues/1800)
- Alpha from vertex colors and textures is ignored. Workaround: set the alpha of `albedo_factor` to make a whole mesh transparent.
    - [#1611](https://github.com/rerun-io/rerun/issues/1611), [#12223](https://github.com/rerun-io/rerun/issues/12223)
- Custom shaders without writing Rust. Workaround: build your own Viewer with a [custom visualizer](../viewer/extend/extend-ui.md#custom-visualizers-for-built-in-views).
    - [#3087](https://github.com/rerun-io/rerun/issues/3087)

### Data types
- Colormaps for `Points3D` and other archetypes with colors (colormaps work for depth images and tensors). Workaround: compute colors in your own code.
    - [#6897](https://github.com/rerun-io/rerun/issues/6897)
- Filled 2D polygons or 2D meshes. Workaround: draw outlines with `LineStrips2D`, or rasterize into a `SegmentationImage`.
    - [#6808](https://github.com/rerun-io/rerun/issues/6808), [#6012](https://github.com/rerun-io/rerun/issues/6012)
- Heading or orientation on `GeoPoints`. Workaround: draw an arrow with `GeoLineStrings`.
    - [#8814](https://github.com/rerun-io/rerun/issues/8814)
- Custom map tile servers. The map view supports OpenStreetMap and Mapbox styles.
    - [#12599](https://github.com/rerun-io/rerun/issues/12599)
- 3D terrain in the map view.
- Non-pinhole camera models and lens distortion. Workaround: undistort images before you log them.
    - [#2315](https://github.com/rerun-io/rerun/issues/2315), [#2499](https://github.com/rerun-io/rerun/issues/2499)
- Heightmap or surface type. Workaround: log a `Mesh3D` grid, or see the Rust [`custom_visualizer`](https://github.com/rerun-io/rerun/tree/main/examples/rust/custom_visualizer) example, which renders a height field.
- 64-bit floats for positions and transforms, to avoid coordinate jitter from f32. Workaround: subtract a local origin before you log.
    - [#4378](https://github.com/rerun-io/rerun/issues/4378)
- Euler angles for rotations. Workaround: convert to a quaternion, for example with `scipy.spatial.transform.Rotation`.
    - [#10962](https://github.com/rerun-io/rerun/issues/10962)

### Logging and SDK
- Resolving world poses from the transform tree in the SDK. Workaround: compose the transforms in your own code.
- Reusing one asset across many entities. Workaround: log the asset once and draw copies with `InstancePoses3D` on the same entity.
    - Related: [assets](../query-and-transform/catalog/catalog-object-model.md#assets) let all segments of a dataset share static data, such as a mesh.
- Automatic deduplication of unchanged values. Workaround: only log a value when it changes; views that show the latest value, such as 3D and 2D views, show it until a new one arrives. Time series plots interpolate linearly between samples, so log the last unchanged sample too if you need a flat line.
    - [#3745](https://github.com/rerun-io/rerun/issues/3745)
- An SDK for `no_std` targets and microcontrollers. Workaround: send data to a host computer and log it from there.
- gRPC connection status from Python, and reconnecting after a dropped connection. The SDK retries the first connection, but not after the connection drops. The Rust `GrpcSink` has a `status()` method.
    - [#2353](https://github.com/rerun-io/rerun/issues/2353), [#7090](https://github.com/rerun-io/rerun/issues/7090)
- Streaming raw PCM audio. `AssetAudio` takes whole encoded files. Workaround: log complete audio files, or plot the samples as `Scalars`.
    - [#2852](https://github.com/rerun-io/rerun/issues/2852)
- Appending to an existing `.rrd` file. Workaround: write one file per session and combine them with `rerun rrd merge`, or open several files at once.
- Tailing an `.rrd` file while it grows. Workaround: use `rr.set_sinks` with both a `FileSink` and a `GrpcSink` to save and stream at the same time.
- A blueprint builder API in C++. Workaround: build the blueprint in Python, save it as an `.rbl` file, and open it with your recording.
    - [#5520](https://github.com/rerun-io/rerun/issues/5520)

### Files and formats
- Importing rosbag2 `.db3` (SQLite) files. Workaround: convert them to MCAP with `ros2 bag convert`, then [import the MCAP](../logging-and-ingestion/import/mcap/working-with-mcap.md).
- Exporting a `VideoStream` or point clouds from the Viewer. (`AssetVideo` and other blobs have a **Save blob…** button in the selection panel.) Workaround: read the data with the [dataframe API](../query-and-transform/dataframes/dataframe-queries.md) and export it yourself.
    - [#12747](https://github.com/rerun-io/rerun/issues/12747) (video)

### Web and server
- Custom request headers (for example auth) when the web Viewer fetches an `.rrd` over HTTP. Workaround: fetch the file yourself and pass the bytes to `openChannel(…).sendRrd(…)` in the JS package. For Rerun servers, set `fallbackToken`.
- Per-client sessions in the `serve_grpc` proxy: every connected Viewer gets the same stream. Workaround: one recording and port per client.
- Watching a folder for new files, and registering data from object stores (S3, GCS) in the open-source `rerun server`. It only registers local files, either at startup with `--dataset` or later with `register`.
- Live streams larger than RAM. Data streamed from the SDK to the Viewer is kept in memory, and the oldest data is dropped at the memory limit. Workaround: save to an `.rrd` file and open the file, which is read on demand (see [Limit memory usage](../viewer/performance/limit-ram.md)).
- More memory in the web Viewer. It runs as 32-bit Wasm and is limited to about 2 GiB in practice. Workaround: use the native Viewer. Large local `.rrd` files are read on demand through the Viewer catalog (**Load files via Viewer catalog**, on by default), but `.rrd` files opened over HTTP are streamed into memory.

### Integrations
- A first-party Bevy integration. [Revy](https://github.com/rerun-io/revy) is a proof of concept.
- ROS-native interactive tools, such as interactive markers or publishing to topics from the Viewer.
