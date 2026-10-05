<!--[metadata]
title = "Custom Viewer callback"
thumbnail = "https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/480w.png"
thumbnail_dimensions = [480, 291]
-->

Advanced example showing how to control an external application from the Rerun viewer, by extending the viewer UI.

<picture>
  <img src="https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/full.png" alt="Custom Viewer Callback example screenshot">
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/custom_callback/1434da408fd59ea1349169784b47d8ffc285022e/1200w.png">
</picture>

> [!NOTE]
> The web version loads `@rerun-io/web-viewer` from this repository, so that the viewer and the SDK have the same version.
> Outside of the Rerun repository, install the package from npm instead, with the same version as your SDK.

## Overview

This example is divided into two parts:

- **Viewer**: The UI that shows the data and has the control panel.
  It comes in two versions:
  - Native ([`src/viewer.rs`](src/viewer.rs)): Wraps the Rerun viewer inside an [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app, with a control panel made in [`egui`](https://github.com/emilk/egui).
  - Web ([`web/`](web/)): Embeds the Rerun web viewer with [`@rerun-io/web-viewer`](../../../rerun_js/web-viewer/), next to a control panel made in plain HTML and JavaScript.
- **App** ([`src/app.rs`](src/app.rs)): The application that uses the Rerun SDK.

In both versions, the `app` does all of the logging, and the control panel only sends it commands.

The communication between the viewer and the `app` is implemented in the [`comms`](src/comms/) module.
It defines a simple set of messages in [`protocol.rs`](src/comms/protocol.rs), such as logging a [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d) or [`Point3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) to an entity, or changing the radius of a set of points that is being logged.

Both control panels send these messages to the same WebSocket, `ws://127.0.0.1:9091/ws`, as JSON, for example `{"DynamicPosition": {"radius": 0.2, "offset": 1.0}}`.
The `app` runs this server in [`comms/app.rs`](src/comms/app.rs), and also serves the browser panel from it.

The recording goes over a separate connection: the `app` serves it on `rerun+http://127.0.0.1:9876/proxy` with `serve_grpc`, and both viewers connect to it.
Because the `app` serves everything, you can use both viewers at the same time.

## Usage

First start the Rerun SDK app with `cargo run -p custom_callback --bin custom_callback_app`.
Then open one or both viewers:

- **Native:** `cargo run -p custom_callback --bin custom_callback_viewer`.
- **Web:** Build the web viewer package once with `pixi run js-build-base`, then open <http://127.0.0.1:9091>.

## Relationship with Viewer callbacks

The [`re_viewer`] crate also exposes some baseline Viewer events through the [`StartupOptions.on_event`](https://docs.rs/re_viewer/latest/re_viewer/struct.StartupOptions.html#structfield.on_event) field,
which can exist alongside your own events from widgets added by extending the UI.
