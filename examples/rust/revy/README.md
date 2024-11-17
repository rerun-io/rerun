<!--[metadata]
title = "Revy - Rerun integration for Bevy"
source = "https://github.com/rerun-io/revy"
tags = ["2D", "3D", "Gamedev", "Bevy"]
thumbnail = "https://static.rerun.io/revy/d451ab9e75a1bcdf140f592feaf15e0cf0041259/480w.png"
thumbnail_dimensions = [480, 480]
-->

<picture>
  <img src="https://static.rerun.io/revy/d451ab9e75a1bcdf140f592feaf15e0cf0041259/full.png" alt="">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy/d451ab9e75a1bcdf140f592feaf15e0cf0041259/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy/d451ab9e75a1bcdf140f592feaf15e0cf0041259/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy/d451ab9e75a1bcdf140f592feaf15e0cf0041259/1024w.png">
</picture>

## Overview

Revy is a proof-of-concept time-travel debugger for the [Bevy](https://github.com/bevyengine/bevy) game engine, built using [Rerun](https://github.com/rerun-io/rerun).

Revy works by snapshotting diffs of the Bevy database every frame that are then logged into the Rerun database.\
This allows you to inspect and visualize the state of the engine at any point in time, either in real-time or after the fact.\
These recordings can then be shared to be replayed or e.g. attached to bug reports.

For more information, check out the [Revy repository](https://github.com/rerun-io/revy).

## Examples

<table>
  <thead>
    <tr>
      <th><a href="https://github.com/bevyengine/bevy/blob/v0.13.0/examples/games/breakout.rs">Breakout</a></th>
      <th><a href="https://github.com/bevyengine/bevy/blob/v0.13.0/examples/3d/3d_shapes.rs">3D shapes</a></th> <!-- NOLINT -->
      <th><a href="https://github.com/bevyengine/bevy/blob/v0.13.0/examples/games/alien_cake_addict.rs">Alien Cake Addict</a></th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>
        <a href="https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/breakout_014_001.rrd">Live demo</a>
        <picture>
          <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/1200w.png">
          <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/1024w.png">
          <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/768w.png">
          <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/480w.png">
          <img src="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/full.png" alt="">
        </picture>
      </td>
      <td>
        <a href="https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/3d_shapes_014_001.rrd">Live demo</a>
        <picture>
          <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/1200w.png">
          <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/1024w.png">
          <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/768w.png">
          <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/480w.png">
          <img src="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/full.png" alt="">
        </picture>
      </td>
      <td>
        <a href="https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/alien_014_001.rrd">Live demo</a>
        <picture>
          <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/1200w.png">
          <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/1024w.png">
          <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/768w.png">
          <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/480w.png">
          <img src="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/full.png" alt="">
        </picture>
      </td>
    </tr>
  </tbody>
</table>

## Usage

1. [Install the Rerun Viewer](https://www.rerun.io/docs/getting-started/installing-viewer) (`0.15`).

2. Add `revy` to your dependencies:
    ```toml
    revy = "0.15"  # always matches the Rerun version
    ```

3. Initialize the `rerun` plugin:
    ```rust
    .add_plugins({
        let rec = revy::RecordingStreamBuilder::new("<your_app_name>").spawn().unwrap();
        revy::RerunPlugin { rec }
    })
    ```
    This will start a Rerun Viewer in the background and stream the recording data to it.\
    Check out the [`RecordingStreamBuilder`](https://ref.rerun.io/docs/rust/stable/rerun/struct.RecordingStreamBuilder.html) docs for other options (saving to file, connecting to a remote viewer, etc).
