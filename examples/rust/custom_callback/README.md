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
> [#2337](https://github.com/rerun-io/rerun/issues/2337): In order to spawn a web viewer with these customizations applied, you have to build the web viewer of the version yourself. This is currently not supported outside of the Rerun repository.

## Overview

This example is divided into two parts:

- **Viewer** ([`src/viewer.rs`](src/viewer.rs)): Wraps the Rerun viewer inside an [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) app.
- **App** ([`src/app.rs`](src/app.rs)): The application that uses the Rerun SDK.

In the `app`, an additional gRPC server is opened to allow the `viewer` to send messages to the `app`.
Similar to the [`extend_viewer_ui`](../extend_viewer_ui/) example, the `viewer` is wrapped in an `eframe` app, which allows us to handle the extra communication logic and define our own control UI using [`egui`](https://github.com/emilk/egui).

The communication between the `viewer` and the `app` is implemented in the [`comms`](src/comms/) module. It defines a simple protocol to send messages between the `viewer` and the `app` using [`bincode`](https://github.com/bincode-org/bincode).
The protocol supports basic commands that the `viewer` can send to the `app`, such as logging a [`Boxes3D`](https://www.rerun.io/docs/reference/types/archetypes/boxes3d) or [`Point3D`](https://www.rerun.io/docs/reference/types/archetypes/points3d) to an entity, or changing the radius of a set of points that is being logged.

## Usage

First start the Rerun SDK app with `cargo run -p custom_callback --bin custom_callback_app`,
and then start the extended viewer with `cargo run -p custom_callback --bin custom_callback_viewer`.

## Relationship with Viewer callbacks

The [`re_viewer`] crate also exposes some baseline Viewer events through the [`StartupOptions.on_event`](https://docs.rs/re_viewer/latest/re_viewer/struct.StartupOptions.html#structfield.on_event) field,
which can exist alongside your own events from widgets added by extending the UI.
